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

fn img_element(img: std::sync::Arc<RenderImage>) -> impl IntoElement {
    gpui_kit::img(img).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain)
}

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
    /// Pictures of items by id: their rows are 40 px with a 70 × 40 picture
    /// before the label (live Form ▾ of the form library, request 1622 (d)).
    pictures: std::collections::HashMap<SharedString, std::sync::Arc<RenderImage>>,
    /// The labels are already in the UI language (a feature dialog's
    /// options, [`crate::i18n::spec`]): drawn as they are.
    translated: bool,
    current: SharedString,
    /// Field height (with its underline).
    height: f32,
    /// The search input while open.
    search: Option<Entity<InputState>>,
    query: String,
    highlight: usize,
    /// Where keyboard focus was before opening, to give it back on close.
    return_focus: Option<FocusHandle>,
    /// The closed field as a Tab stop (Onshape's off-screen
    /// `os-select-focusser`), created by [`OsSelect::focus_handle`].
    focus: Option<FocusHandle>,
    /// The field's and the open list's painted bounds (the list matches the
    /// field's width; presses inside the list don't close it).
    bounds: Rc<Cell<Bounds<Pixels>>>,
    list_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// The open list is longer than it can show (draws the scrollbar).
    list_overflow: Rc<Cell<bool>>,
    /// Scroll of the open list (at most [`LIST_MAX_H`] tall).
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<OsSelectEvent> for OsSelect {}

impl OsSelect {
    /// Mark the labels as already in the UI language (not looked up again).
    pub fn set_translated(&mut self, on: bool) {
        self.translated = on;
    }

    pub fn new(id: impl Into<SharedString>) -> Self {
        Self { id: id.into(), items: Vec::new(), pictures: Default::default(), translated: false, current: SharedString::default(), height: FIELD_H, search: None, query: String::new(), highlight: 0, return_focus: None, focus: None, bounds: Default::default(), list_bounds: Default::default(), list_overflow: Default::default(), scroll: ScrollHandle::new(), _subs: Vec::new() }
    }

    /// A shorter field (Hole's lookup rows are 22 px).
    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }

    /// Pictures shown in the list's rows, by item id.
    pub fn set_pictures(&mut self, pictures: std::collections::HashMap<SharedString, std::sync::Arc<RenderImage>>) {
        self.pictures = pictures;
    }

    /// The items and the current one (call on every host render).
    pub fn set_items(&mut self, items: Vec<(SharedString, SharedString)>, current: impl Into<SharedString>) {
        self.items = items;
        self.current = current.into();
    }

    /// The select's Tab stop (made on first use). Onshape's focusser: Enter,
    /// Space, ↑ or ↓ open the list, a typed character opens it searching for
    /// that character, and closing the list gives the focus back to it.
    pub fn focus_handle(&mut self, cx: &mut Context<Self>) -> FocusHandle {
        self.focus.get_or_insert_with(|| cx.focus_handle().tab_stop(true)).clone()
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
        // A search box: Esc closes the list (below), it doesn't revert the query.
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false));
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
        self.scroll.scroll_to_item(self.highlight + 1);
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.take().is_none() {
            return;
        }
        self._subs.clear();
        self.query.clear();
        // Onshape (osSelect:close): the focus goes back to the focusser.
        let back = self.return_focus.take();
        if let Some(f) = self.focus.clone().or(back) {
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

    fn render_closed(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let label = self.items.iter().find(|(id, _)| *id == self.current).map(|(_, l)| l.clone()).unwrap_or_default();
        let focused = self.focus.as_ref().is_some_and(|f| f.is_focused(window));
        let mut el = h_flex().id(self.id.clone());
        if let Some(f) = &self.focus {
            el = el
                .track_focus(f)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    let k = &ev.keystroke;
                    let plain = !k.modifiers.control && !k.modifiers.alt && !k.modifiers.platform;
                    match k.key.as_str() {
                        "space" | "down" | "up" => this.open(window, cx),
                        // Typing opens the list with that text (focusser keyup).
                        _ if plain && k.key_char.as_ref().is_some_and(|c| c.chars().count() == 1 && !c.chars().all(char::is_whitespace)) => {
                            let c = k.key_char.clone().unwrap_or_default();
                            this.open(window, cx);
                            if let Some(st) = this.search.clone() {
                                st.update(cx, |s, cx| s.set_value(c.clone(), window, cx));
                            }
                            this.query = c;
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                }))
                // Enter opens it too (enterActivate), before the dialog's Accept.
                .capture_action(cx.listener(|this, _: &crate::actions::Accept, window, cx| {
                    if this.focus.as_ref().is_some_and(|f| f.is_focused(window)) {
                        cx.stop_propagation();
                        this.open(window, cx);
                    }
                }));
        }
        el
            .w_full()
            .h(px(self.height))
            // Live (dialogs/chamfer-01): the value and caret sit 1 px below
            // the field's centre, the caret 5 px from the end.
            .pt(px(2.))
            .pl(px(6.))
            .pr(px(4.))
            .border_b_1()
            .border_color(pal.border)
            // `#def1fb` hover (§4.5).
            .hover(|d| d.bg(pal.row_hover))
            .cursor_pointer()
            .child(div().flex_1().min_w_0().text_size(px(14.)).text_color(pal.text).truncate().map(|d| if self.translated { d.child(crate::ui::user_text(label)) } else { d.child(label) }))
            // Live: the 8x4 bootstrap caret, 6 px from the end.
            .child(crate::ui::glyph("drawing-tool-caret", 12., pal.text))
            // Focused (bootstrap .btn-default-focus inside os-enum-parameter):
            // the soft blue glow, no outline.
            .when(focused, |d| d.shadow(vec![BoxShadow { color: hsla(207. / 360., 0.74, 0.66, 0.6), offset: point(px(0.), px(0.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }]))
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
        let pictured = !self.pictures.is_empty();
        let rows = m.into_iter().enumerate().map(|(row, (i, range))| {
            let label = self.items[i].1.clone();
            let picture = self.pictures.get(&self.items[i].0).cloned();
            let on = row == hl;
            let text: AnyElement = match range {
                Some(r) => StyledText::new(label).with_highlights([(r, HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })]).into_any_element(),
                None if self.translated => crate::ui::user_text(label).into_any_element(),
                None => label.into_any_element(),
            };
            div()
                .id(row)
                .h(px(if pictured { 40. } else { ROW_H }))
                // Rows keep their height: a long list scrolls (live 7½ rows).
                .flex_none()
                .flex()
                .items_center()
                .pl(px(if pictured { 4. } else { TEXT_INSET }))
                .when(pictured, |d| d.gap(px(8.)).child(div().w(px(70.)).h(px(40.)).flex_none().children(picture.map(|img| img_element(img)))))
                .text_size(px(14.))
                .when(on, |d| d.bg(accent).text_color(gpui_kit::rgb_static(0xffffff)))
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
        // Live (assembly/36, dialogs/hole-33): a list longer than 200 px
        // scrolls with Chrome's scrollbar on its right edge; the frame
        // holds the scrollbar, the inner column scrolls.
        let overflow = self.list_overflow.get();
        let (scroll, flag) = (self.scroll.clone(), self.list_overflow.clone());
        let bar_w = gpui_kit::component::menu::NATIVE_SCROLLBAR_WIDTH;
        deferred(
            anchored().snap_to_window_with_margin(px(4.)).child(
                div()
                    .id("os-select-list")
                    .occlude()
                    .w(width)
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_1()
                    .border_color(pal.border)
                    .font_family(cx.theme().font_family.clone())
                    .relative()
                    .child(
                        v_flex()
                            .id("os-select-items")
                            .w_full()
                            .max_h(px(LIST_MAX_H - 2.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .when(overflow, |d| d.pr(px(bar_w)))
                            // The insets are rows: padding on a scroll container
                            // counts as overflow, and a 3-item list showed a
                            // scrollbar (qa4 F31, icons2).
                            .child(div().h(px(LIST_PAD_TOP)).flex_none())
                            .children(rows)
                            .child(div().h(px(LIST_PAD_BOTTOM)).flex_none()),
                    )
                    .when(overflow, |d| d.child(gpui_kit::component::menu::native_scrollbar(&self.scroll, cx)))
                    .on_drag_move(cx.listener(|this, e: &DragMoveEvent<gpui_kit::component::menu::NativeThumbDrag>, window, cx| {
                        gpui_kit::component::menu::native_scrollbar_drag(&this.scroll, e.event.position.y);
                        window.refresh();
                        cx.notify();
                    }))
                    .child(
                        canvas(
                            move |b, window, _| {
                                probe.set(b);
                                // Known once laid out: draw again with the scrollbar.
                                let o = scroll.max_offset().y > px(0.5);
                                if o != flag.get() {
                                    flag.set(o);
                                    window.request_animation_frame();
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            ),
        )
        .with_priority(3)
        .into_any_element()
    }
}

impl Render for OsSelect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let probe = self.bounds.clone();
        let accent = gpui_kit::rgb(0x1651b0);
        let field = match &self.search {
            None => self.render_closed(window, cx),
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
            // Esc with the list closed goes on to the dialog (cancel; qa5 SM9).
            .on_action(cx.listener(|this, _: &SelectDismiss, window, cx| {
                if this.search.is_some() {
                    this.close(window, cx);
                } else {
                    cx.propagate();
                }
            }))
            // Esc with the list open closes only the list, whichever binding
            // won (the dialog's Esc = Cancel binding can outrank ours when the
            // app's keys are bound again later): caught before the dialog.
            .capture_action(cx.listener(|this, _: &crate::actions::Cancel, window, cx| {
                if this.search.is_some() {
                    cx.stop_propagation();
                    this.close(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &gpui_kit::component::input::Escape, window, cx| {
                if this.search.is_some() {
                    this.close(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| {
                let n = this.matches().len();
                if n > 0 {
                    this.highlight = (this.highlight.min(n - 1) + 1).min(n - 1);
                    this.scroll.scroll_to_item(this.highlight + 1);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| {
                let n = this.matches().len();
                this.highlight = this.highlight.min(n.saturating_sub(1)).saturating_sub(1);
                this.scroll.scroll_to_item(this.highlight + 1);
                cx.notify();
            }))
            // Tab leaves the open list.
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
