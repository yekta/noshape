//! Onshape's right-click menu (live DOM, dom/context-menus-partstudio.txt):
//! white, 1 px `#ccc` outline, radius 4, shadow `0 5px 10px rgba(0,0,0,.2)`,
//! 1 px padding; 22 px rows of 12 px `#333` text starting 36 px from the
//! left, a 16 px full-color icon 11 px from the left, hover `#def1fb`,
//! 1 px `#ddd` separators across the whole width, and submenus (▸) that
//! open beside their row.
//!
//! The builder API mirrors the kit's `PopupMenu` (`item`, `separator`,
//! `submenu`), so a menu builder switches by changing its import.
//! [`OsContextMenuExt::os_context_menu`] opens one on right-click; the
//! window root draws it with [`render_host`].

use crate::theme::p;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

/// Row height and text size (live).
const ROW_H: f32 = 22.;
const TEXT: f32 = 12.;
/// Text inset from the menu's left edge, and the icon's.
const TEXT_X: f32 = 36.;
const ICON_X: f32 = 11.;
const ICON: f32 = 16.;

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

actions!(os_menu, [MenuUp, MenuDown, MenuLeft, MenuRight, MenuConfirm, MenuDismiss]);

/// Menu keys, bound in the menu's own key context so they win over the
/// window's bindings (Esc = Cancel, arrows = rotate the view).
fn bind_keys(cx: &mut App) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let ctx = Some("OsMenu");
        cx.bind_keys([
            KeyBinding::new("up", MenuUp, ctx),
            KeyBinding::new("down", MenuDown, ctx),
            KeyBinding::new("left", MenuLeft, ctx),
            KeyBinding::new("right", MenuRight, ctx),
            KeyBinding::new("enter", MenuConfirm, ctx),
            KeyBinding::new("escape", MenuDismiss, ctx),
        ]);
    });
}

pub enum MenuIcon {
    /// A full-color app icon (`assets::icon`).
    Named(SharedString),
    /// A kit icon (single color).
    Kit(Box<Icon>),
}

/// One row of an [`OsMenu`].
pub enum OsMenuItem {
    Item { label: SharedString, icon: Option<MenuIcon>, checked: bool, disabled: bool, handler: Option<Handler> },
    Separator,
    Submenu { label: SharedString, icon: Option<MenuIcon>, menu: Entity<OsMenu> },
}

impl OsMenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self::Item { label: label.into(), icon: None, checked: false, disabled: false, handler: None }
    }

    /// A single-color kit icon at the left.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        if let Self::Item { icon: i, .. } | Self::Submenu { icon: i, .. } = &mut self {
            *i = Some(MenuIcon::Kit(Box::new(icon.into())));
        }
        self
    }

    /// A full-color app icon at the left (Onshape's menus show the tool icons).
    pub fn icon_name(mut self, name: &str) -> Self {
        if let Self::Item { icon: i, .. } | Self::Submenu { icon: i, .. } = &mut self {
            *i = Some(MenuIcon::Named(SharedString::from(name.to_string())));
        }
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        if let Self::Item { handler, .. } = &mut self {
            *handler = Some(Rc::new(f));
        }
        self
    }

    fn clickable(&self) -> bool {
        matches!(self, Self::Item { disabled: false, .. } | Self::Submenu { .. })
    }
}

/// An Onshape right-click menu. Emits [`DismissEvent`] when an item was
/// clicked (also from a submenu), on Esc and on a click outside.
pub struct OsMenu {
    items: Vec<OsMenuItem>,
    focus: FocusHandle,
    hovered: Option<usize>,
    /// Painted bounds of this menu and of its open submenu (outside clicks).
    bounds: Rc<Cell<Bounds<Pixels>>>,
    _subs: Vec<Subscription>,
    /// The menu this one is a submenu of (Left returns to it).
    parent: Option<WeakEntity<OsMenu>>,
}

impl EventEmitter<DismissEvent> for OsMenu {}

impl Focusable for OsMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl OsMenu {
    pub fn build(window: &mut Window, cx: &mut App, f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self) -> Entity<Self> {
        bind_keys(cx);
        cx.new(|cx| f(Self { items: Vec::new(), focus: cx.focus_handle(), hovered: None, bounds: Rc::new(Cell::new(Bounds::default())), _subs: Vec::new(), parent: None }, window, cx))
    }

    pub fn item(mut self, item: impl Into<OsMenuItem>) -> Self {
        self.items.push(item.into());
        self
    }

    /// A separator (never two in a row, never first).
    pub fn separator(mut self) -> Self {
        if self.items.last().is_some_and(|i| !matches!(i, OsMenuItem::Separator)) {
            self.items.push(OsMenuItem::Separator);
        }
        self
    }

    pub fn submenu(mut self, label: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self) -> Self {
        let menu = Self::build(window, cx, f);
        let me = cx.weak_entity();
        menu.update(cx, |m, _| m.parent = Some(me));
        // A click in the submenu closes the whole menu.
        self._subs.push(cx.subscribe(&menu, |_, _, _: &DismissEvent, cx| cx.emit(DismissEvent)));
        self.items.push(OsMenuItem::Submenu { label: label.into(), icon: None, menu });
        self
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn open_submenu(&self) -> Option<&Entity<OsMenu>> {
        match self.hovered.and_then(|i| self.items.get(i)) {
            Some(OsMenuItem::Submenu { menu, .. }) => Some(menu),
            _ => None,
        }
    }

    /// Whether `pos` is over this menu or its open submenus.
    fn contains(&self, pos: Point<Pixels>, cx: &App) -> bool {
        self.bounds.get().contains(&pos) || self.open_submenu().is_some_and(|m| m.read(cx).contains(pos, cx))
    }

    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(OsMenuItem::Item { disabled: false, handler, .. }) = self.items.get(ix) {
            let handler = handler.clone();
            cx.emit(DismissEvent);
            if let Some(h) = handler {
                h(&ClickEvent::default(), window, cx);
            }
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.items.len() as isize;
        if n == 0 {
            return;
        }
        let mut i = self.hovered.map(|i| i as isize).unwrap_or(if delta > 0 { -1 } else { n });
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
            if self.items[i as usize].clickable() {
                self.hovered = Some(i as usize);
                cx.notify();
                return;
            }
        }
    }

    fn open_into_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(m) = self.open_submenu().cloned() {
            m.update(cx, |m, cx| m.step(1, cx));
            window.focus(&m.focus_handle(cx), cx);
        }
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.hovered.and_then(|i| self.items.get(i)) {
            Some(OsMenuItem::Submenu { .. }) => self.open_into_submenu(window, cx),
            Some(_) => {
                if let Some(i) = self.hovered {
                    self.activate(i, window, cx);
                }
            }
            None => {}
        }
    }
}

impl Render for OsMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let hover_bg = gpui_kit::rgb(0xdef1fb);
        let bounds = self.bounds.clone();
        let menu_bounds = bounds.get();
        // Top of each row inside the menu (1 px border + 1 px padding).
        let row_tops: Vec<f32> = self
            .items
            .iter()
            .scan(2.0f32, |y, i| {
                let top = *y;
                *y += if matches!(i, OsMenuItem::Separator) { 1. } else { ROW_H };
                Some(top)
            })
            .collect();
        let rows: Vec<AnyElement> = self
            .items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let hovered = self.hovered == Some(ix);
                let icon_el = |icon: &Option<MenuIcon>, cx: &App| -> Option<AnyElement> {
                    icon.as_ref().map(|i| match i {
                        MenuIcon::Named(n) => crate::assets::icon(n, ICON, cx).into_any_element(),
                        MenuIcon::Kit(k) => (**k).clone().with_size(px(ICON)).text_color(pal.text).into_any_element(),
                    })
                };
                let row = |id: usize| {
                    h_flex()
                        .id(id)
                        .relative()
                        .h(px(ROW_H))
                        .w_full()
                        .pl(px(TEXT_X))
                        .pr(px(12.))
                        .text_size(px(TEXT))
                        .whitespace_nowrap()
                        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                            if *h && this.hovered != Some(ix) {
                                this.hovered = Some(ix);
                                cx.notify();
                            }
                        }))
                };
                match item {
                    OsMenuItem::Separator => div().h(px(1.)).w_full().bg(gpui_kit::rgb(0xdddddd)).into_any_element(),
                    OsMenuItem::Item { label, icon, checked, disabled, .. } => row(ix)
                        .text_color(if *disabled { pal.text_faint } else { pal.text })
                        .when(hovered && !*disabled, |d| d.bg(hover_bg))
                        .when_some(icon_el(icon, cx), |d, i| d.child(div().absolute().left(px(ICON_X)).top(px((ROW_H - ICON) * 0.5)).size(px(ICON)).child(i)))
                        .when(*checked && icon.is_none(), |d| {
                            d.child(div().absolute().left(px(ICON_X)).top(px((ROW_H - ICON) * 0.5)).size(px(ICON)).flex().items_center().justify_center().child(Icon::new(gpui_kit::component::IconName::Check).xsmall().text_color(pal.text)))
                        })
                        .child(label.clone())
                        .when(!*disabled, |d| {
                            d.cursor_pointer().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate(ix, window, cx)))
                        })
                        .into_any_element(),
                    OsMenuItem::Submenu { label, icon, menu } => row(ix)
                        .text_color(pal.text)
                        .when(hovered, |d| d.bg(hover_bg))
                        .when_some(icon_el(icon, cx), |d, i| d.child(div().absolute().left(px(ICON_X)).top(px((ROW_H - ICON) * 0.5)).size(px(ICON)).child(i)))
                        .child(div().flex_1().child(label.clone()))
                        // The small ▸ at the right (live: a 4 px black triangle).
                        .child(div().ml(px(16.)).child(crate::ui::glyph("ui-caret-right", 8., pal.text)))
                        .when(hovered, |d| {
                            // Opens beside the row: its top-left at the menu's right edge,
                            // level with the row (window coordinates from the last paint).
                            let at = point(menu_bounds.right() - px(2.), menu_bounds.top() + px(row_tops[ix] - 1.));
                            d.child(deferred(anchored().position(at).snap_to_window_with_margin(px(4.)).child(div().occlude().child(menu.clone()))).with_priority(3))
                        })
                        .into_any_element(),
                }
            })
            .collect();
        v_flex()
            .id("os-menu")
            .key_context("OsMenu")
            .track_focus(&self.focus)
            .occlude()
            .on_action(cx.listener(|this, _: &MenuDown, _, cx| this.step(1, cx)))
            .on_action(cx.listener(|this, _: &MenuUp, _, cx| this.step(-1, cx)))
            .on_action(cx.listener(|this, _: &MenuRight, window, cx| this.open_into_submenu(window, cx)))
            .on_action(cx.listener(|this, _: &MenuConfirm, window, cx| this.confirm(window, cx)))
            // Esc closes the menu, like Onshape; Left leaves a submenu.
            .on_action(cx.listener(|_, _: &MenuDismiss, _, cx| cx.emit(DismissEvent)))
            .on_action(cx.listener(|this, _: &MenuLeft, window, cx| {
                if let Some(parent) = this.parent.as_ref().and_then(|p| p.upgrade()) {
                    this.hovered = None;
                    window.focus(&parent.focus_handle(cx), cx);
                    cx.notify();
                }
            }))
            .on_mouse_down_out(cx.listener(|this, e: &MouseDownEvent, _, cx| {
                if !this.contains(e.position, cx) {
                    cx.emit(DismissEvent);
                }
            }))
            .on_hover(cx.listener(|this, h: &bool, _, cx| {
                // Leaving the menu clears the row highlight unless a submenu is open.
                if !*h && this.open_submenu().is_none() && this.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .p(px(1.))
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(0xcccccc))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .children(rows)
            .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().size_full())
    }
}

/// The open right-click menu of a window (see [`render_host`]).
#[derive(Default)]
struct OsMenuHost(Option<(Point<Pixels>, Entity<OsMenu>, Subscription)>);

impl Global for OsMenuHost {}

/// Show `menu` at `position` (window coordinates) until it is dismissed.
pub fn open(position: Point<Pixels>, menu: Entity<OsMenu>, window: &mut Window, cx: &mut App) {
    let sub = cx.subscribe(&menu, |_, _: &DismissEvent, cx| {
        cx.default_global::<OsMenuHost>().0 = None;
        cx.refresh_windows();
    });
    window.focus(&menu.focus_handle(cx), cx);
    cx.default_global::<OsMenuHost>().0 = Some((position, menu, sub));
    window.refresh();
}

/// The open menu, for the window root to draw on top of everything.
pub fn render_host(cx: &App) -> Option<AnyElement> {
    let (pos, menu, _) = cx.try_global::<OsMenuHost>()?.0.as_ref()?;
    Some(deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(menu.clone())).with_priority(2).into_any_element())
}

/// Right-click opens an Onshape menu built by `f` at the pointer.
pub trait OsContextMenuExt: InteractiveElement + Sized {
    fn os_context_menu(self, f: impl Fn(OsMenu, &mut Window, &mut Context<OsMenu>) -> OsMenu + 'static) -> Self {
        let f = Rc::new(f);
        self.on_mouse_down(MouseButton::Right, move |e, window, cx| {
            cx.stop_propagation();
            let f = f.clone();
            let menu = OsMenu::build(window, cx, move |m, window, cx| f(m, window, cx));
            if !menu.read(cx).is_empty() {
                open(e.position, menu, window, cx);
            }
        })
    }
}

impl<E: InteractiveElement + Sized> OsContextMenuExt for E {}
