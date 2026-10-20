//! Onshape's right-click menu (live DOM, dom/context-menus-partstudio.txt):
//! white, 1 px `#ccc` outline, radius 4, shadow `0 5px 10px rgba(0,0,0,.2)`,
//! 1 px padding; 22 px rows of 12 px `#333` text starting 36 px from the
//! left, a 16 px full-color icon 11 px from the left, hover `#def1fb`,
//! 1 px `#ddd` separators across the whole width, and submenus (▸) that
//! open beside their row.
//!
//! The builder API mirrors the kit's `PopupMenu` (`item`, `separator`,
//! `submenu`), so a menu builder switches by changing its import.
//! [`OsContextMenuExt::os_context_menu`] opens one on right-click; each
//! window opened with [`open_window`] draws its own open menu.

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

/// Paint order of open menus: the kit's popup priority (gpui-base
/// `POPUP_PRIORITY`), so menus opened from a dialog (selects) draw above it.
const POPUP_PRIORITY: usize = 100;

pub enum MenuIcon {
    /// A full-color app icon (`assets::icon`).
    Named(SharedString),
    /// A kit icon (single color).
    Kit(Box<Icon>),
}

/// One row of an [`OsMenu`].
pub enum OsMenuItem {
    Item { label: SharedString, icon: Option<MenuIcon>, checked: bool, disabled: bool, handler: Option<Handler>, keys: Vec<SharedString> },
    Separator,
    Submenu { label: SharedString, icon: Option<MenuIcon>, menu: Entity<OsMenu> },
}

impl OsMenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self::Item { label: label.into(), icon: None, checked: false, disabled: false, handler: None, keys: Vec::new() }
    }

    /// Shortcut key chips at the right ("shift", "ctrl", "s"), as Onshape's
    /// Feature Studio Commit menu.
    pub fn keys(mut self, chips: &[&str]) -> Self {
        if let Self::Item { keys, .. } = &mut self {
            *keys = chips.iter().map(|k| SharedString::from(k.to_string())).collect();
        }
        self
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

    /// A ✓ at the icon position (camera menu choices).
    pub fn checked(mut self, on: bool) -> Self {
        if let Self::Item { checked, .. } = &mut self {
            *checked = on;
        }
        self
    }

    /// Gray and not clickable.
    pub fn disabled(mut self, on: bool) -> Self {
        if let Self::Item { disabled, .. } = &mut self {
            *disabled = on;
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
    /// Where keyboard focus was before the menu opened (root menu only); it
    /// goes back there when the menu closes, so shortcuts keep working.
    return_focus: Option<FocusHandle>,
    /// Onshape's icon-less list menus (documents page, Bootstrap
    /// `list-no-icons`): 13 px text 17 px from the edge.
    plain: bool,
    /// Feature Studio snippet menus: 33 px rows, 14 px `#333` text 14 px in.
    snippets: bool,
    /// Onshape's select lists (e.g. Variable type): this width, 26 px rows,
    /// 14 px `#666` text 10 px in; the hovered (else the checked) row is
    /// `#1651b0` with white text.
    select: Option<f32>,
    /// With `select`: a browser `<select>` list instead (the account pages,
    /// live preferences 62, assembly/182): 27 px rows, 14 px `#333` text,
    /// `#1967d2` highlight, 1 px `#767676` border, no shadow.
    native: bool,
    /// Onshape's drawing toolbar lists: 26 px rows, 18 px icons 4 px in,
    /// 13.33 px text 30 px in.
    toolbar: bool,
    /// Onshape's Bootstrap dropdowns (the "+" add-element menu): 14 px `#555`
    /// text 43 px in, 27 px rows, 20 px icons 20 px in, radius 5.25, `#d7d7d7`.
    dropdown: bool,
    /// The camera and render options menu (live dom/menu-camera-render):
    /// 222 px rows of 22 px, 12 px `#555` text 27 px in, 16 px icons 6 px in.
    camera: bool,
    /// One-shot Esc handler: the first Esc runs it and the menu stays open
    /// (a sketch tool's menu: Esc exits the tool), the next one dismisses.
    on_escape: Option<EscapeHandler>,
    /// A native `<select>`'s field while its list is open: Chrome draws it
    /// with a 2 px `#3e94ff` focus ring (live assembly/182, navigation/62).
    /// Drawn by the menu host, so it shows exactly while the list is open.
    ring: Option<Bounds<Pixels>>,
}

/// An `OsMenu`'s one-shot Esc handler.
type EscapeHandler = Rc<dyn Fn(&mut Window, &mut App)>;

impl EventEmitter<DismissEvent> for OsMenu {}

impl Focusable for OsMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl OsMenu {
    pub fn build(window: &mut Window, cx: &mut App, f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self) -> Entity<Self> {
        bind_keys(cx);
        cx.new(|cx| f(Self { items: Vec::new(), focus: cx.focus_handle(), hovered: None, bounds: Rc::new(Cell::new(Bounds::default())), _subs: Vec::new(), parent: None, return_focus: None, plain: false, snippets: false, select: None, native: false, toolbar: false, dropdown: false, camera: false, on_escape: None, ring: None }, window, cx))
    }

    /// Select-list style, `width` wide (the field's width).
    pub fn select_list(mut self, width: f32) -> Self {
        self.select = Some(width);
        self
    }

    /// A browser `<select>` list, `width` wide (see `native`). Open it with
    /// [`open_anchored`] at the field's bottom-left; mark the current value
    /// `checked` (it is highlighted until the pointer moves over a row).
    pub fn native_select(mut self, width: f32) -> Self {
        self.select = Some(width);
        self.native = true;
        self
    }

    /// Ring `field` (window coordinates) while the list is open (a native
    /// `<select>`'s focus ring).
    pub fn focus_ring(mut self, field: Bounds<Pixels>) -> Self {
        self.ring = Some(field);
        self
    }

    /// Drawing toolbar dropdown style.
    pub fn toolbar_list(mut self) -> Self {
        self.toolbar = true;
        self
    }

    /// The camera and render options menu's style.
    pub fn camera(mut self) -> Self {
        self.camera = true;
        self
    }

    /// Bootstrap dropdown style ("+" menu).
    pub fn dropdown(mut self) -> Self {
        self.dropdown = true;
        self
    }

    /// Run `f` on the first Esc instead of dismissing (see `on_escape`).
    pub fn on_first_escape(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_escape = Some(Rc::new(f));
        self
    }

    /// Feature Studio snippet menus (live document/71): 33 px rows, 14 px
    /// `#333` text 14 px in, no icons.
    pub fn snippet_list(mut self) -> Self {
        self.snippets = true;
        self
    }

    /// Icon-less list style (13 px text, 17 px inset), as on the documents page.
    pub fn list_no_icons(mut self) -> Self {
        self.plain = true;
        self
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

    pub fn submenu(self, label: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self) -> Self {
        self.submenu_with_icon(None, label, window, cx, f)
    }

    /// A submenu row with an icon (the camera menu's ✓ rows).
    pub fn submenu_with_icon(mut self, icon: Option<Icon>, label: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self) -> Self {
        let menu = Self::build(window, cx, f);
        let me = cx.weak_entity();
        menu.update(cx, |m, _| m.parent = Some(me));
        // A click in the submenu closes the whole menu.
        self._subs.push(cx.subscribe(&menu, |_, _, _: &DismissEvent, cx| cx.emit(DismissEvent)));
        self.items.push(OsMenuItem::Submenu { label: label.into(), icon: icon.map(|i| MenuIcon::Kit(Box::new(i))), menu });
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

    /// Close the menu (and its parents). Afterwards, unless something else
    /// took keyboard focus (a dialog the item opened, the clicked element),
    /// focus returns to where it was before the menu opened.
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let mut root_focus = self.focus.clone();
        let mut ret = self.return_focus.clone();
        let mut parent = self.parent.as_ref().and_then(|p| p.upgrade());
        while let Some(pm) = parent {
            let pm = pm.read(cx);
            root_focus = pm.focus.clone();
            ret = pm.return_focus.clone();
            parent = pm.parent.as_ref().and_then(|p| p.upgrade());
        }
        let own = self.focus.clone();
        cx.emit(DismissEvent);
        if let Some(ret) = ret.clone() {
            window.defer(cx, move |window, cx| {
                let stale = match window.focused(cx) {
                    None => true,
                    Some(f) => f == own || f == root_focus,
                };
                if stale {
                    window.focus(&ret, cx);
                }
            });
        }
        ret
    }

    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(OsMenuItem::Item { disabled: false, handler, .. }) = self.items.get(ix) {
            let handler = handler.clone();
            // Focus goes back first, so a prompt the item opens returns there too.
            let ret = self.dismiss(window, cx);
            if let Some(ret) = ret {
                window.focus(&ret, cx);
            }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let dropdown = self.dropdown;
        let select = self.select;
        let native = self.native;
        let no_hover = self.hovered.is_none();
        let any_icon = self.items.iter().any(|i| matches!(i, OsMenuItem::Item { icon: Some(_), .. } | OsMenuItem::Submenu { icon: Some(_), .. }));
        // In dropdowns a ✓ takes the icon place too (documents Folders sort menu).
        let any_check = self.items.iter().any(|i| matches!(i, OsMenuItem::Item { checked: true, .. }));
        let snippets = self.snippets;
        let (row_h, icon_x, icon_sz, text_x, text_sz): (f32, f32, f32, f32, f32) = if snippets {
            (33., 4., 18., 14., 14.)
        } else if dropdown {
            (27., 20., 20., if any_icon || any_check { 43. } else { 20. }, 14.)
        } else if native {
            (27., 6., 18., 10., 14.)
        } else if self.camera {
            (ROW_H, 6., ICON, 27., 12.)
        } else if self.select.is_some() {
            // Selects with icons (assembly mate type) put the text after the icon.
            (26., 6., 18., if any_icon { 30. } else { 10. }, 14.)
        } else if self.toolbar {
            (26., 4., 18., 30., 13.33)
        } else if self.plain {
            (ROW_H, ICON_X, ICON, 17., 13.)
        } else {
            // Live: text 36 px in when an item has an icon, else 17 px.
            (ROW_H, ICON_X, ICON, if any_icon || any_check { TEXT_X } else { 17. }, TEXT)
        };
        let plain_menu = !dropdown && !snippets && select.is_none() && !self.toolbar;
        let camera = self.camera;
        let window_w = window.viewport_size().width;
        let hover_bg = gpui_kit::rgb(0xdef1fb);
        let bounds = self.bounds.clone();
        let menu_bounds = bounds.get();
        // Top of each row inside the menu (1 px border; context menus have
        // no padding, rows and separators run edge to edge).
        let row_tops: Vec<f32> = self
            .items
            .iter()
            .scan(if plain_menu { 1.0f32 } else { 2.0 }, |y, i| {
                let top = *y;
                *y += if matches!(i, OsMenuItem::Separator) { if dropdown { 15. } else { 1. } } else { row_h };
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
                        MenuIcon::Named(n) => crate::assets::icon(n, icon_sz, cx).into_any_element(),
                        MenuIcon::Kit(k) => div().size(px(icon_sz)).flex().items_center().justify_center().child((**k).clone().with_size(px(12.)).text_color(pal.text)).into_any_element(),
                    })
                };
                // "Label\tbeta": the label and a blue badge (live drawing/447:
                // "Select for replication [beta]").
                let label_el = |label: &SharedString| -> AnyElement {
                    match label.split_once('\t') {
                        Some((text, badge)) => h_flex()
                            .child(text.to_string())
                            .child(div().ml(px(15.)).px(px(6.)).h(px(20.)).flex().items_center().rounded(px(3.)).bg(gpui_kit::rgb(0x1651b0)).text_color(gpui_kit::rgb(0xffffff)).text_size(px(12.)).child(badge.to_string()))
                            .into_any_element(),
                        None => label.clone().into_any_element(),
                    }
                };
                let row = |id: usize| {
                    h_flex()
                        .id(id)
                        .relative()
                        .h(px(row_h))
                        .w_full()
                        .pl(px(text_x))
                        .pr(px(if plain_menu { 10. } else { 12. }))
                        .text_size(px(text_sz))
                        .whitespace_nowrap()
                        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                            if *h && this.hovered != Some(ix) {
                                this.hovered = Some(ix);
                                cx.notify();
                            }
                        }))
                };
                match item {
                    // Bootstrap dropdown dividers have 7 px margins.
                    OsMenuItem::Separator => div().h(px(1.)).w_full().when(dropdown, |d| d.my(px(7.))).bg(gpui_kit::rgb(0xdddddd)).into_any_element(),
                    OsMenuItem::Item { label, icon, checked, disabled, keys, .. } => row(ix)
                        .text_color(if *disabled { pal.text_faint } else if snippets { pal.text } else if dropdown || camera { gpui_kit::rgb(0x555555) } else if select.is_some() && !native { gpui_kit::rgb(0x666666) } else { pal.text })
                        .when(hovered && !*disabled && select.is_none(), |d| d.bg(hover_bg))
                        .when(select.is_some() && (hovered || (no_hover && *checked)), |d| d.bg(gpui_kit::rgb(if native { 0x1967d2 } else { 0x1651b0 })).text_color(gpui_kit::rgb(0xffffff)))
                        .when_some(icon_el(icon, cx), |d, i| d.child(div().absolute().left(px(icon_x)).top(px((row_h - icon_sz) * 0.5)).size(px(icon_sz)).child(i)))
                        .when(*checked && icon.is_none() && select.is_none(), |d| {
                            d.child(div().absolute().left(px(icon_x)).top(px((row_h - icon_sz) * 0.5)).size(px(icon_sz)).flex().items_center().justify_center().child(Icon::new(gpui_kit::component::IconName::Check).xsmall().text_color(pal.text)))
                        })
                        .child(label_el(label))
                        .when(!keys.is_empty(), |d| {
                            let chip = |k: &SharedString| div().px(px(3.)).h(px(18.)).flex().items_center().rounded(px(2.)).border_1().border_color(gpui_kit::rgb(0x333333)).text_size(px(12.)).text_color(pal.text).child(k.clone());
                            d.child(div().flex_1().min_w(px(16.))).child(h_flex().gap(px(2.)).children(keys.iter().map(chip)))
                        })
                        .when(!*disabled, |d| {
                            d.cursor_pointer().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate(ix, window, cx)))
                        })
                        .into_any_element(),
                    OsMenuItem::Submenu { label, icon, menu } => row(ix)
                        .text_color(pal.text)
                        .when(hovered, |d| d.bg(hover_bg))
                        .when_some(icon_el(icon, cx), |d, i| d.child(div().absolute().left(px(icon_x)).top(px((row_h - icon_sz) * 0.5)).size(px(icon_sz)).child(i)))
                        .child(div().flex_1().child(label_el(label)))
                        // The small ▸ at the right (live: a 5 px `#666` triangle 9 px
                        // from the border).
                        .pr(px(6.))
                        .child(div().ml(px(16.)).child(crate::ui::glyph("ui-caret-right", 10., gpui_kit::rgb(0x666666))))
                        .when(hovered, |d| {
                            // Opens beside the row: its top-left at the menu's right edge,
                            // level with the row (window coordinates from the last paint).
                            // Near the window's right edge it opens to the left (live camera menu).
                            // Live: the submenu's border lies on the parent's.
                            let y = menu_bounds.top() + px(row_tops[ix] - 1.);
                            let (at, anchor) = if menu_bounds.right() + px(200.) > window_w {
                                (point(menu_bounds.left() + px(1.), y), Anchor::TopRight)
                            } else {
                                (point(menu_bounds.right() - px(1.), y), Anchor::TopLeft)
                            };
                            d.child(deferred(anchored().anchor(anchor).position(at).snap_to_window_with_margin(px(4.)).child(div().occlude().child(menu.clone()))).with_priority(POPUP_PRIORITY + 1))
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
            .on_action(cx.listener(|this, _: &MenuDismiss, window, cx| match this.on_escape.take() {
                Some(f) => f(window, cx),
                None => {
                    this.dismiss(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MenuLeft, window, cx| {
                if let Some(parent) = this.parent.as_ref().and_then(|p| p.upgrade()) {
                    this.hovered = None;
                    window.focus(&parent.focus_handle(cx), cx);
                    cx.notify();
                }
            }))
            .on_mouse_down_out(cx.listener(|this, e: &MouseDownEvent, window, cx| {
                if !this.contains(e.position, cx) {
                    this.dismiss(window, cx);
                }
            }))
            .on_hover(cx.listener(|this, h: &bool, _, cx| {
                // Leaving the menu clears the row highlight unless a submenu is open.
                if !*h && this.open_submenu().is_none() && this.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .when(!plain_menu, |d| d.p(px(1.)))
            .when(plain_menu, |d| d.min_w(px(120.)))
            .when(camera, |d| d.w(px(224.)))
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(if dropdown { 0xd7d7d7 } else { 0xcccccc }))
            .rounded(px(if dropdown { 5.25 } else { 4. }))
            .when(dropdown, |d| d.py(px(4.)).min_w(px(253.)).text_color(gpui_kit::rgb(0x555555)))
            .when_some(select, |d, w| d.w(px(w)).rounded(px(0.)).p_0())
            // 0 5px 10px rgba(0,0,0,.2); GPUI blurs twice as far as CSS.
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }])
            .when(native, |d| d.shadow(Vec::new()).py(px(1.)).border_color(gpui_kit::rgb(0x767676)))
            .text_color(pal.text)
            .children(rows)
            .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
    }
}

/// The open right-click menu and the window it belongs to (see
/// [`render_host`]); one menu is open at a time, app-wide.
#[derive(Default)]
struct OsMenuHost(Option<OpenMenu>);

/// An open menu: its window, position and anchor corner, and the dismiss
/// subscription.
type OpenMenu = (WindowId, Point<Pixels>, Anchor, Entity<OsMenu>, Subscription);

impl Global for OsMenuHost {}

/// Show `menu` at `position` (window coordinates) until it is dismissed.
pub fn open(position: Point<Pixels>, menu: Entity<OsMenu>, window: &mut Window, cx: &mut App) {
    open_anchored(position, Anchor::TopLeft, menu, window, cx);
}

/// Show `menu` with its `anchor` corner at `position` (a dropdown under a
/// button uses `TopRight` at the button's bottom-right corner).
pub fn open_anchored(position: Point<Pixels>, anchor: Anchor, menu: Entity<OsMenu>, window: &mut Window, cx: &mut App) {
    let sub = cx.subscribe(&menu, |_, _: &DismissEvent, cx| {
        cx.default_global::<OsMenuHost>().0 = None;
        cx.refresh_windows();
    });
    let prev = window.focused(cx);
    menu.update(cx, |m, _| m.return_focus = prev);
    window.focus(&menu.focus_handle(cx), cx);
    let id = window.window_handle().window_id();
    let prev = cx.default_global::<OsMenuHost>().0.replace((id, position, anchor, menu, sub));
    if prev.is_some_and(|p| p.0 != id) {
        // A menu still open in another window goes away.
        cx.refresh_windows();
    }
    window.refresh();
}

/// A right-click menu is open (hover tooltips under it should not show).
pub fn is_open(cx: &App) -> bool {
    cx.try_global::<OsMenuHost>().is_some_and(|h| h.0.is_some())
}

/// The menu open in `window`, drawn on top of everything. Windows opened
/// with [`open_window`] draw it themselves.
fn render_host(window: &Window, cx: &App) -> Vec<AnyElement> {
    let Some((id, pos, anchor, menu, _)) = cx.try_global::<OsMenuHost>().and_then(|h| h.0.as_ref()) else { return Vec::new() };
    if *id != window.window_handle().window_id() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(b) = menu.read(cx).ring {
        let ring = div().w(b.size.width).h(b.size.height).rounded(px(5.25)).border_2().border_color(gpui_kit::rgb(0x3e94ff));
        out.push(deferred(anchored().position(b.origin).child(ring)).with_priority(POPUP_PRIORITY).into_any_element());
    }
    // Live: a menu that doesn't fit below the pointer opens above it.
    out.push(deferred(anchored().anchor(*anchor).position(*pos).child(menu.clone())).with_priority(POPUP_PRIORITY).into_any_element());
    out
}

/// The layer every app window's content sits in: the content plus the
/// window's open [`OsMenu`].
struct MenuLayer(AnyView);

impl Render for MenuLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone()).children(render_host(window, cx))
    }
}

/// The content view of a window opened with [`open_window`] (under the
/// kit's Root and the menu layer), e.g. to find a window's Workspace.
pub fn content_view(window: &Window, cx: &App) -> Option<AnyView> {
    let root = window.root::<gpui_kit::base::Root>().flatten()?;
    let view = root.read(cx).view().clone();
    match view.clone().downcast::<MenuLayer>() {
        Ok(layer) => Some(layer.read(cx).0.clone()),
        Err(_) => Some(view),
    }
}

/// `gpui_kit::open_window` with the Onshape menus wired in: right-click and
/// dropdown menus opened in this window draw in it (above its dialogs).
/// Open every app window with this.
pub fn open_window<V: Render>(options: WindowOptions, cx: &mut App, build: impl FnOnce(&mut Window, &mut App) -> Entity<V>) -> anyhow::Result<(AnyWindowHandle, Entity<V>)> {
    let mut built = None;
    let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
        let view = build(window, cx);
        built = Some(view.clone());
        cx.new(|_| MenuLayer(view.into()))
    })?;
    Ok((handle, built.expect("open_window ran its build closure")))
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
                open(e.position + point(px(0.), px(2.)), menu, window, cx);
            }
        })
    }

    /// [`os_context_menu`](Self::os_context_menu) taken in the capture phase,
    /// so it wins over a text input inside the element, whose own Cut / Copy /
    /// Paste menu would open instead: Onshape's table rows show the row menu
    /// over their cells, focused or not (live variable-studio/01–07).
    fn os_context_menu_over_inputs(self, f: impl Fn(OsMenu, &mut Window, &mut Context<OsMenu>) -> OsMenu + 'static) -> Self {
        let f = Rc::new(f);
        self.capture_any_mouse_down(move |e, window, cx| {
            if e.button != MouseButton::Right {
                return;
            }
            cx.stop_propagation();
            let f = f.clone();
            let menu = OsMenu::build(window, cx, move |m, window, cx| f(m, window, cx));
            if !menu.read(cx).is_empty() {
                open(e.position + point(px(0.), px(2.)), menu, window, cx);
            }
        })
    }
}

impl<E: InteractiveElement + Sized> OsContextMenuExt for E {}
