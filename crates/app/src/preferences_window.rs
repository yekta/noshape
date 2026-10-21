//! Onshape's "My account › Preferences" page as a window (live:
//! shortcuts/32-47, dom/customize-keyboard-shortcuts.txt): the account
//! header, the left navigation (the local pages Profile and Preferences),
//! and the Preferences sections that apply locally: Mouse controls
//! (mouse_prefs.rs), Part Studio settings,
//! Assembly settings, Keyboard shortcuts (tabs, the 472x456 list with
//! Edit / Reset on hover, the legend, Reset all, the "Edit shortcut"
//! popup) and Shortcut toolbars. Opened from the documents page's account
//! menu and from "Customize keyboard shortcuts" in the Shift+/ sheet
//! (scrolled to Keyboard shortcuts, like Onshape's
//! `/user/settings?focusView=customize-keyboard-shortcuts`).

use crate::actions::CloseWindow;
use crate::os_ui as os;
use crate::part_studio::toolbar_prefs::ToolbarPrefs;
use crate::shortcuts::{self, Entry, KeyStyle, Recorded, Tab};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, IconName, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};

const CONTEXT: &str = "Preferences";
/// Left navigation width (live 220 px incl. its 1 px border).
const NAV_W: f32 = 220.;
/// Content inset from the navigation (live: headings' rule starts at 245).
const CONTENT_PAD: f32 = 25.;
/// Keyboard shortcut list (live 472x456).
const LIST_W: f32 = 472.;
const LIST_H: f32 = 456.;
/// "Edit shortcut" popup width.
const POPUP_W: f32 = 600.;
/// Onshape's account page blue-gray rules (`#d0dbe5`).
const RULE: u32 = 0xd0dbe5;
/// Key chip outline (`span.key`: 1 px `#aaa`, 2 px at the bottom).
const CHIP_BORDER: u32 = 0xaaaaaa;

// ----------------------------------------------------------------- prefs

/// Part Studio / Assembly settings of the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPrefs {
    /// "Display parts list properties" (Part Studio parts list).
    #[serde(default)]
    pub parts_list_properties: bool,
    /// "Display instance list properties" (Assembly instance list).
    #[serde(default)]
    pub instance_list_properties: bool,
    /// Drawings › "Background color of model space": Light view (default Dark).
    #[serde(default)]
    pub drawing_light_model_space: bool,
}

fn list_prefs_path() -> std::path::PathBuf {
    crate::files::config_dir().join("preferences.json")
}

impl ListPrefs {
    pub fn load() -> Self {
        std::fs::read_to_string(list_prefs_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(list_prefs_path(), t);
        }
    }
}

/// Whether Part Studios show parts list properties (Preferences › Part
/// Studio settings; the parts list's "Show/Hide parts list properties"
/// changes it too).
pub fn parts_list_properties() -> bool {
    ListPrefs::load().parts_list_properties
}

pub fn set_parts_list_properties(on: bool) {
    let mut p = ListPrefs::load();
    p.parts_list_properties = on;
    p.save();
}

pub fn set_instance_list_properties(on: bool) {
    let mut p = ListPrefs::load();
    p.instance_list_properties = on;
    p.save();
}

// ----------------------------------------------------------------- window

/// The page (left navigation) and where to scroll.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Profile,
    Preferences,
    /// Preferences, scrolled to Keyboard shortcuts.
    KeyboardShortcuts,
    /// Preferences, scrolled to User export rules (the Export dialogs' "View
    /// export rules"; live: the section after Material libraries).
    ExportRules,
}

struct PrefsWindow {
    handle: AnyWindowHandle,
    view: WeakEntity<PreferencesView>,
}
impl Global for PrefsWindow {}

/// Opens the page (or brings the open one to the front) at `page`.
pub fn show(page: Page, cx: &mut App) {
    if let Some(g) = cx.try_global::<PrefsWindow>() {
        let (handle, view) = (g.handle, g.view.clone());
        let shown = handle.update(cx, |_, window, cx| {
            window.activate_window();
            let _ = view.update(cx, |v, cx| v.go_to(page, cx));
        });
        if shown.is_ok() {
            return;
        }
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1280.), px(900.)), cx))),
        window_min_size: Some(size(px(900.), px(560.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..TitleBar::window_options()
    };
    match crate::os_menu::open_window(options, cx, |window, cx| {
        window.set_window_title("My account — noshape");
        let view = cx.new(|cx| PreferencesView::new(page, window, cx));
        let fh = view.read(cx).focus.clone();
        window.defer(cx, move |window, cx| fh.focus(window, cx));
        view
    }) {
        Ok((handle, view)) => cx.set_global(PrefsWindow { handle, view: view.downgrade() }),
        Err(e) => log::error!("could not open the preferences window: {e:#}"),
    }
}

/// Called when a window closed: forgets the page if it was that one.
pub fn forget_if(id: WindowId, cx: &mut App) -> bool {
    let is_prefs = cx.try_global::<PrefsWindow>().is_some_and(|g| g.handle.window_id() == id);
    if is_prefs {
        cx.remove_global::<PrefsWindow>();
    }
    is_prefs
}

/// The open "Edit shortcut" popup.
struct EditShortcut {
    entry: &'static Entry,
    /// The combination pressed (canonical), not yet confirmed.
    pressed: Option<String>,
    /// "Disable shortcut", and its state when the popup opened.
    disable: bool,
    was_disabled: bool,
    /// The ⓘ popover (invalid keys) is shown.
    info: bool,
    /// Takes the key presses while the popup is open.
    _keys: Subscription,
}

impl EditShortcut {
    fn changed(&self) -> bool {
        self.pressed.is_some() || self.disable != self.was_disabled
    }
}

pub struct PreferencesView {
    focus: FocusHandle,
    page: Page,
    scroll: ScrollHandle,
    saved: ListPrefs,
    edited: ListPrefs,
    tab: Tab,
    list_scroll: ScrollHandle,
    edit: Option<EditShortcut>,
    /// Scroll Keyboard shortcuts into view once laid out (as the browser's
    /// `focusView`: its top at the top, or as far as the page scrolls).
    scroll_to_shortcuts: bool,
    /// Scroll to the last sections (User export rules) once laid out.
    scroll_to_end: bool,
    /// Shortcut toolbars: the context shown (0 = Sketch, 1 = Part Studio,
    /// 2 = Assembly).
    toolbar_context: usize,
    /// The assembly's S toolbar (skb, assembly_toolbar.rs): saved and draft.
    asm_toolbar: crate::assembly_view::AsmToolbarPrefs,
    asm_toolbar_draft: crate::assembly_view::AsmToolbarPrefs,
    /// Saved toolbar preferences, and the Shortcut toolbars draft that
    /// "Save shortcut toolbar settings" writes (live: ticks apply on Save).
    toolbar: ToolbarPrefs,
    toolbar_draft: ToolbarPrefs,
    name: Entity<InputState>,
    name_saved: String,
    /// Release management and Material libraries sections (release_prefs.rs).
    release: Entity<crate::release_prefs::ReleasePrefsView>,
    /// Mouse controls section (mouse_prefs.rs).
    mouse: Entity<crate::mouse_prefs::MousePrefsView>,
    /// Units section (units_prefs.rs).
    units: Entity<crate::units_prefs::UnitsPrefsView>,
    /// Decimal format section (units_prefs.rs).
    decimal: Entity<crate::units_prefs::DecimalPrefsView>,
    /// Drawings section (drawing_prefs.rs).
    drawings: Entity<crate::drawing_prefs::DrawingPrefsView>,
    _subs: Vec<Subscription>,
}

impl Focusable for PreferencesView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PreferencesView {
    fn new(page: Page, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_saved = crate::files::user_name_pref().unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(noshape_store::os_login()).default_value(name_saved.clone()));
        let subs = vec![cx.subscribe_in(&name, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })];
        let saved = ListPrefs::load();
        let mut this = Self {
            focus: cx.focus_handle(),
            page,
            scroll: ScrollHandle::new(),
            saved,
            edited: saved,
            tab: Tab::General,
            list_scroll: ScrollHandle::new(),
            edit: None,
            scroll_to_shortcuts: false,
            scroll_to_end: false,
            toolbar_context: 0,
            toolbar: ToolbarPrefs::load(),
            toolbar_draft: ToolbarPrefs::load(),
            asm_toolbar: crate::assembly_view::AsmToolbarPrefs::load(),
            asm_toolbar_draft: crate::assembly_view::AsmToolbarPrefs::load(),
            name,
            name_saved,
            release: cx.new(|cx| crate::release_prefs::ReleasePrefsView::new(window, cx)),
            mouse: cx.new(|_| crate::mouse_prefs::MousePrefsView::new()),
            units: cx.new(|_| crate::units_prefs::UnitsPrefsView::new()),
            decimal: cx.new(|_| crate::units_prefs::DecimalPrefsView::default()),
            drawings: cx.new(|_| crate::drawing_prefs::DrawingPrefsView::new()),
            _subs: subs,
        };
        this.go_to(page, cx);
        this
    }

    fn go_to(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        match page {
            // The sections are the scroll area's children: Keyboard shortcuts is the fourth.
            Page::KeyboardShortcuts => self.scroll_to_shortcuts = true,
            Page::ExportRules => self.scroll_to_end = true,
            _ => self.scroll.set_offset(point(px(0.), px(0.))),
        }
        cx.notify();
    }

    // ------------------------------------------------------------ edit popup

    fn open_edit(&mut self, entry: &'static Entry, window: &mut Window, cx: &mut Context<Self>) {
        let was_disabled = shortcuts::current().is_disabled(entry);
        let view = cx.entity().downgrade();
        let wid = window.window_handle().window_id();
        // The focused box takes every key press (also those bound to
        // commands, like Ctrl+Q), as the browser's input does.
        let keys = cx.intercept_keystrokes(move |ev, window, cx| {
            if window.window_handle().window_id() != wid {
                return;
            }
            let Some(view) = view.upgrade() else { return };
            cx.stop_propagation();
            let k = ev.keystroke.clone();
            view.update(cx, |this, cx| this.on_key(&k, cx));
        });
        self.edit = Some(EditShortcut { entry, pressed: None, disable: was_disabled, was_disabled, info: false, _keys: keys });
        cx.notify();
    }

    fn on_key(&mut self, k: &Keystroke, cx: &mut Context<Self>) {
        match shortcuts::record(k) {
            Recorded::Key(key) => {
                if let Some(ed) = &mut self.edit {
                    ed.pressed = Some(key);
                }
                cx.notify();
            }
            Recorded::Confirm => self.confirm_edit(cx),
            Recorded::Cancel => self.close_edit(cx),
            Recorded::Ignored => {}
        }
    }

    /// ✓ / Enter: saves right away (greyed until something changed).
    fn confirm_edit(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.edit.as_ref().filter(|e| e.changed()) else { return };
        let (entry, key, disable) = (ed.entry, ed.pressed.clone(), ed.disable);
        shortcuts::update(cx, |s| {
            s.assign(entry, key, disable);
        });
        self.close_edit(cx);
    }

    fn close_edit(&mut self, cx: &mut Context<Self>) {
        self.edit = None;
        cx.notify();
    }

    fn reset_entry(&mut self, entry: &'static Entry, cx: &mut Context<Self>) {
        shortcuts::update(cx, |s| s.reset(entry));
        cx.notify();
    }

    fn confirm_reset_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit = None;
        // Onshape: a Bootstrap modal near the top.
        window.open_dialog(cx, |dialog, _, cx| {
            os::modal(dialog, "Reset shortcuts to defaults?", 600.)
                .child(os::modal_body().child("Are you sure you want to reset all keyboard shortcuts to default values?"))
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("kb-reset-all-ok", "Reset"))).child(os::modal_cancel(os::cancel_button("kb-reset-all-cancel", "Cancel", cx))))
                .on_ok(|_, window, cx| {
                    shortcuts::update(cx, |s| s.reset_all());
                    window.refresh();
                    true
                })
        });
        cx.notify();
    }

    // ------------------------------------------------------------ rendering

    fn render_title_bar(&self, cx: &App) -> AnyElement {
        let pal = p();
        TitleBar::new()
            .h(px(crate::ui::NAVBAR_H))
            .bg(pal.panel_inactive)
            .border_b_1()
            .border_color(pal.border)
            .child(
                h_flex()
                    .w_full()
                    .h_full()
                    .child(h_flex().gap(px(5.)).pl(px(8.)).child(crate::assets::icon("noshape-logo", 22., cx)).child(div().text_size(px(21.)).font_weight(FontWeight::LIGHT).text_color(pal.text_muted).child("noshape")))
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .pr(px(14.))
                            .gap(px(5.))
                            .child(crate::assets::icon("user-avatar", 22., cx))
                            .child(div().text_size(px(14.)).text_color(pal.text).max_w(px(180.)).truncate().child(noshape_store::local_user_name())),
                    ),
            )
            .into_any_element()
    }

    fn render_header(&self) -> AnyElement {
        let pal = p();
        h_flex()
            .h(px(60.))
            .flex_none()
            .pl(px(10.))
            .gap(px(15.))
            .border_b_1()
            .border_color(rgb(RULE))
            .text_size(px(24.))
            .child(div().font_weight(FontWeight::MEDIUM).text_color(pal.accent).child("My account"))
            .child(div().text_color(pal.text_muted).child(if self.page == Page::Profile { "Profile" } else { "Preferences" }))
            .into_any_element()
    }

    fn render_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let item = |id: &'static str, label: &'static str, page: Page, active: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(27.))
                .pl(px(10.))
                .flex()
                .items_center()
                // Live navigation/61: 14 px.
                .text_size(px(14.))
                .text_color(pal.text)
                .cursor_pointer()
                .when(active, |d| d.bg(rgb(0xb2ddf6)).font_weight(FontWeight::BOLD))
                .when(!active, |d| d.hover(|d| d.text_color(pal.accent)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.go_to(page, cx)))
        };
        let on_profile = self.page == Page::Profile;
        v_flex()
            .w(px(NAV_W))
            .h_full()
            .flex_none()
            .pt(px(10.))
            .border_r_1()
            .border_color(rgb(RULE))
            .child(item("nav-profile", "Profile", Page::Profile, on_profile, cx))
            .child(item("nav-preferences", "Preferences", Page::Preferences, !on_profile, cx))
            .into_any_element()
    }

    /// A section heading: 18 px / 500 over a 1 px `#333` rule.
    fn heading(title: &'static str, extra: Option<AnyElement>) -> Div {
        let pal = p();
        v_flex().child(
            h_flex()
                .h(px(32.))
                .pl(px(15.))
                .gap(px(6.))
                .border_b_1()
                .border_color(pal.text)
                .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title))
                .children(extra),
        )
    }

    /// A browser checkbox (13 px, `#767676` outline) with its 13.33 px label.
    fn checkbox(id: &'static str, label: &'static str, on: bool) -> Stateful<Div> {
        let pal = p();
        h_flex()
            .id(id)
            .h(px(27.))
            .gap(px(4.))
            .cursor_pointer()
            .text_size(px(13.33))
            .text_color(pal.text)
            .child(
                div()
                    .size(px(13.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .border_1()
                    .border_color(if on { rgb(0x0075ff) } else { rgb(0x767676) })
                    .bg(if on { rgb(0x0075ff) } else { rgb(0xffffff) })
                    .when(on, |d| d.child(Icon::new(IconName::Check).size(px(11.)).text_color(rgb(0xffffff)))),
            )
            .child(label)
    }

    /// "Save … settings": Onshape's primary button, faded until changed.
    fn save_button(id: &'static str, label: &'static str, enabled: bool) -> Stateful<Div> {
        let pal = p();
        div()
            .id(id)
            .h(px(34.))
            .px(px(11.))
            .flex()
            .items_center()
            .rounded(px(3.))
            .bg(if enabled { pal.accent } else { rgb(0xc6d5ec) })
            .text_size(px(14.))
            .text_color(rgb(0xffffff))
            .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(pal.accent_hover)))
            .child(label)
    }

    fn render_list_settings(&self, cx: &mut Context<Self>) -> (AnyElement, AnyElement) {
        let ps = self.edited.parts_list_properties;
        let asm = self.edited.instance_list_properties;
        let ps_changed = ps != self.saved.parts_list_properties;
        let asm_changed = asm != self.saved.instance_list_properties;
        let part_studio = Self::heading("Part Studio settings", None)
            .child(div().mt(px(14.)).pl(px(15.)).child(Self::checkbox("pref-parts-props", "Display parts list properties", ps).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.edited.parts_list_properties = !this.edited.parts_list_properties;
                cx.notify();
            }))))
            .child(h_flex().mt(px(25.)).pl(px(15.)).child(Self::save_button("pref-save-ps", "Save Part Studio settings", ps_changed).when(ps_changed, |b| {
                b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    set_parts_list_properties(this.edited.parts_list_properties);
                    this.saved.parts_list_properties = this.edited.parts_list_properties;
                    cx.notify();
                }))
            })))
            .into_any_element();
        let assembly = Self::heading("Assembly settings", None)
            .mt(px(38.))
            .child(div().mt(px(14.)).pl(px(15.)).child(Self::checkbox("pref-instance-props", "Display instance list properties", asm).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.edited.instance_list_properties = !this.edited.instance_list_properties;
                cx.notify();
            }))))
            .child(h_flex().mt(px(25.)).pl(px(15.)).child(Self::save_button("pref-save-asm", "Save assembly settings", asm_changed).when(asm_changed, |b| {
                b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    set_instance_list_properties(this.edited.instance_list_properties);
                    this.saved.instance_list_properties = this.edited.instance_list_properties;
                    cx.notify();
                }))
            })))
            .into_any_element();
        // The checkboxes sit 15 px in, under the heading text.
        (div().child(part_studio).into_any_element(), div().child(assembly).into_any_element())
    }

    fn render_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let s = shortcuts::current();
        let tabs: Vec<AnyElement> = Tab::CUSTOMIZE.iter().map(|&t| {
            let on = t == self.tab;
            div()
                .id(SharedString::from(format!("kb-tab-{}", t.label())))
                .h_full()
                .px(px(5.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(14.7))
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).border_b_3().pt(px(3.)).border_color(pal.accent))
                .when(!on, |d| d.hover(|d| d.text_color(pal.accent)))
                .child(t.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.tab = t;
                    this.edit = None;
                    this.list_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }))
                .into_any_element()
        }).collect();
        let rows: Vec<AnyElement> = shortcuts::entries(self.tab).map(|e| self.render_row(e, &s, cx)).collect();
        let list = div()
            .id("kb-list")
            .mt(px(11.))
            .w(px(LIST_W))
            .h(px(LIST_H))
            .border_1()
            .border_color(pal.border)
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .child(v_flex().pt(px(6.)).pb(px(6.)).children(rows))
            .vertical_scrollbar(&self.list_scroll);
        let left = v_flex()
            .w(px(LIST_W))
            .flex_none()
            .child(h_flex().h(px(35.)).gap(px(15.)).items_end().border_b_1().border_color(pal.border).children(tabs))
            .child(list);
        let right = v_flex()
            .ml(px(20.))
            .flex_none()
            .child(h_flex().child(os::primary_button("kb-reset-all", "Reset all").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.confirm_reset_all(window, cx)))))
            .child(Self::legend(cx));
        let help = div().id("kb-help").child(crate::assets::icon("help-gray", 18., cx)).into_any_element();
        let popup = self.render_popup(window, cx);
        Self::heading("Keyboard shortcuts", Some(help))
            .mt(px(38.))
            .relative()
            .child(h_flex().mt(px(16.)).pl(px(15.)).items_start().child(left).child(right))
            .children(popup)
            .into_any_element()
    }

    /// Customized / Onshape default / Disabled / Has conflict / *.
    fn legend(cx: &App) -> AnyElement {
        let pal = p();
        let row = |chip: AnyElement, label: &'static str| h_flex().gap(px(5.)).child(chip).child(div().text_size(px(13.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(label));
        v_flex()
            .mt(px(20.))
            .w(px(210.))
            .pt(px(15.))
            .pb(px(14.))
            .px(px(12.))
            .gap(px(14.))
            .border_1()
            .border_color(pal.text)
            .rounded(px(4.))
            .child(row(key_chip("example", KeyStyle::Customized).into_any_element(), "Customized"))
            .child(row(key_chip("example", KeyStyle::Default).into_any_element(), "Onshape default"))
            .child(row(key_chip("example", KeyStyle::Disabled).into_any_element(), "Disabled"))
            .child(row(crate::assets::icon("shortcut-conflict", 18., cx), "Has conflict").mt(px(-4.)))
            .child(h_flex().mt(px(-4.)).gap(px(5.)).text_size(px(13.33)).text_color(pal.text).child("*").child(div().font_weight(FontWeight::SEMIBOLD).child("applies in drawing sketches")))
            .into_any_element()
    }

    fn render_row(&self, e: &'static Entry, s: &shortcuts::Shortcuts, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let style = s.style(e);
        let key = s.key(e);
        let chips: Vec<AnyElement> = match &key {
            Some(k) => shortcuts::key_chips(k).into_iter().map(|c| key_chip(c, style).into_any_element()).collect(),
            None => vec![key_chip("Unassigned", style).into_any_element()],
        };
        let customized = s.is_customized(e);
        let group = SharedString::from(format!("kb-row-{}", e.id));
        h_flex()
            .id(SharedString::from(format!("kb-{}", e.id)))
            .group(group.clone())
            .h(px(if key.is_some() { 37. } else { 35. }))
            .flex_none()
            .child(h_flex().w(px(179.)).flex_none().justify_end().gap(px(4.)).children(chips))
            .child(div().ml(px(13.)).text_size(px(14.)).text_color(pal.text).child(e.name))
            .when(s.has_conflict(e), |d| d.child(div().ml(px(6.)).child(crate::assets::icon("shortcut-conflict", 16., cx))))
            .child(
                div()
                    .id(SharedString::from(format!("kb-edit-{}", e.id)))
                    .ml(px(8.))
                    .cursor_pointer()
                    .opacity(0.)
                    .group_hover(group.clone(), |d| d.opacity(1.))
                    .child(crate::assets::icon("shortcut-edit", 18., cx))
                    .tooltip(crate::ui::tooltip("Edit"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_edit(e, window, cx))),
            )
            .when(customized, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("kb-reset-{}", e.id)))
                        .ml(px(12.))
                        .cursor_pointer()
                        .opacity(0.)
                        .group_hover(group, |d| d.opacity(1.))
                        .child(crate::assets::icon("shortcut-reset", 18., cx))
                        .tooltip(crate::ui::tooltip("Reset"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.reset_entry(e, cx))),
                )
            })
            .into_any_element()
    }

    /// The 600 px "Edit shortcut: <name>" popup, at the top of the
    /// Keyboard shortcuts section, centered in the window.
    fn render_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ed = self.edit.as_ref()?;
        let pal = p();
        let s = shortcuts::current();
        let entry = ed.entry;
        let changed = ed.changed();
        let conflict = ed.pressed.as_ref().and_then(|k| s.conflict_for(entry, k));
        let can_reset = s.is_customized(entry);
        let width = f32::from(window.viewport_size().width);
        let left = ((width - POPUP_W) / 2.0 - NAV_W - CONTENT_PAD).max(0.0);
        let text = ed.pressed.as_deref().map(shortcuts::key_text).unwrap_or_default();
        // ✓: green `#009400` (faded until something changed), ✕: `#bd3039`.
        let ok = div()
            .id("kb-edit-ok")
            .w(px(28.))
            .h(px(25.))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x009400))
            .when(!changed, |d| d.opacity(0.2))
            .when(changed, |d| d.cursor_pointer())
            .child(Icon::new(IconName::Check).size(px(18.)).text_color(rgb(0xffffff)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm_edit(cx)));
        let close = div()
            .id("kb-edit-cancel")
            .size(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(os::glyph(CROSS, 14., rgb(0xbd3039)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_edit(cx)));
        let field = div()
            .w(px(196.))
            .h(px(34.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .border_2()
            .border_color(pal.focus)
            .text_size(px(14.))
            .text_color(pal.text)
            .child(text);
        let info = div()
            .id("kb-edit-info")
            .relative()
            .ml(px(6.))
            .child(crate::assets::icon("info", 18., cx))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if let Some(ed) = &mut this.edit {
                    ed.info = *hovered;
                }
                cx.notify();
            }))
            .when(ed.info, |d| d.child(invalid_keys_popover(cx)));
        let echo = ed.pressed.as_ref().map(|k| h_flex().mt(px(12.)).h(px(26.)).justify_center().gap(px(4.)).children(shortcuts::key_chips(k).into_iter().map(|c| key_chip(c, KeyStyle::Customized))));
        let alert = conflict.map(|o| {
            h_flex()
                .mt(px(12.))
                .mx(px(10.))
                .min_h(px(54.))
                .px(px(6.))
                .py(px(6.))
                .items_start()
                .gap(px(2.))
                .bg(rgb(0xfff8e6))
                .border_1()
                .border_color(rgb(0xfcda7f))
                .rounded(px(3.))
                .child(div().pt(px(2.)).child(crate::assets::icon("warning", 18., cx)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(14.))
                        .line_height(px(21.))
                        .text_color(pal.text)
                        .child(StyledText::new(format!("This combination is already in use by {0}. Confirming the changes will disable the shortcut {0}.", o.name)).with_highlights(bold_names(o.name))),
                )
        });
        let disable = ed.disable;
        let footer = h_flex()
            .mt(px(if ed.pressed.is_some() { 22. } else { 18. }))
            .h(px(22.))
            .px(px(10.))
            .child(Self::checkbox("kb-edit-disable", "Disable shortcut", disable).text_size(px(14.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(ed) = &mut this.edit {
                    ed.disable = !ed.disable;
                }
                cx.notify();
            })))
            .child(div().flex_1())
            .child(
                h_flex()
                    .id("kb-edit-reset")
                    .gap(px(6.))
                    .text_size(px(14.))
                    .text_color(pal.text_faint)
                    .when(!can_reset, |d| d.opacity(0.6))
                    .when(can_reset, |d| d.cursor_pointer().hover(|d| d.text_color(pal.text_muted)))
                    .child("Reset to default")
                    .child(crate::assets::icon("shortcut-reset", 18., cx))
                    .when(can_reset, |d| {
                        d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.reset_entry(entry, cx);
                            this.close_edit(cx);
                        }))
                    }),
            );
        Some(
            v_flex()
                .id("kb-edit-popup")
                .absolute()
                .top(px(15.))
                .left(px(left))
                .w(px(POPUP_W))
                .pt(px(41.))
                .pb(px(17.))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xd2d2d2))
                .shadow_lg()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_edit(cx)))
                .child(h_flex().absolute().top(px(10.)).right(px(14.)).gap(px(8.)).child(ok).child(close))
                .child(div().h(px(30.)).flex().justify_center().items_center().text_size(px(20.)).text_color(pal.text).child(format!("Edit shortcut: {}", entry.name)))
                .child(div().mt(px(3.)).h(px(20.)).flex().justify_center().items_center().text_size(px(14.7)).text_color(pal.text).child("Press desired key combination and confirm"))
                .child(h_flex().mt(px(22.)).justify_center().child(div().w(px(24.))).child(field).child(info))
                .children(echo)
                .children(alert)
                .child(footer)
                .into_any_element(),
        )
    }

    fn render_toolbars(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        // Live: Sketch (the default) | Part Studio | Assembly | Drawing.
        let contexts = vec!["Sketch".to_string(), "Part Studio".to_string(), "Assembly".to_string(), "Drawing".to_string()];
        let select = os::select_field("tb-context", contexts, self.toolbar_context, {
            let view = cx.entity().downgrade();
            move |i, _, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.toolbar_context = i;
                    cx.notify();
                });
            }
        })
        .w(px(150.));
        // Tools that can be on the 5x2 S toolbar (id, name, icon), and the ones shown.
        use crate::part_studio::shortcut_cmds as sc;
        let (items, shown): (Vec<(String, String, SharedString)>, Vec<String>) = if self.toolbar_context == 3 {
            use crate::drawing_keys as dk;
            let all = dk::SHORTCUT_CATALOGUE.iter().map(|t| (t.0.to_string(), t.0.to_string(), crate::assets::icon_path(t.1))).collect();
            (all, self.toolbar_draft.drawing_shortcut_tools(&dk::DEFAULT_SHORTCUT_TOOLS))
        } else if self.toolbar_context == 2 {
            let all = crate::assembly_view::shortcut_catalog().into_iter().map(|(id, name, icon)| (id.to_string(), name.to_string(), crate::assets::icon_path(icon))).collect();
            (all, self.asm_toolbar_draft.shortcut_tools())
        } else if self.toolbar_context == 1 {
            let all = sc::toolbar_feature_ids()
                .into_iter()
                .filter_map(|id| noshape_model::features::get(id).map(|t| (id.to_string(), t.name.to_string(), crate::part_studio::feature_icon(id))))
                .collect();
            (all, self.toolbar_draft.shortcut_tools())
        } else {
            let all = sc::sketch_toolbar_tools().into_iter().map(|(name, icon)| (name.to_string(), name.to_string(), crate::assets::icon_path(icon))).collect();
            (all, self.toolbar_draft.sketch_shortcut_tools(&sc::default_sketch_shortcut_names()))
        };
        let context = self.toolbar_context;
        let changed = self.toolbar_draft.shortcut_tools != self.toolbar.shortcut_tools || self.toolbar_draft.sketch_shortcut_tools != self.toolbar.sketch_shortcut_tools || self.asm_toolbar_draft.shortcut_tools != self.asm_toolbar.shortcut_tools || self.toolbar_draft.drawing_shortcut_tools != self.toolbar.drawing_shortcut_tools;
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(i, (id, name, icon))| {
                let on = shown.contains(id);
                let id2 = id.clone();
                h_flex()
                    .id(SharedString::from(format!("tb-item-{i}")))
                    .h(px(26.))
                    .pl(px(6.))
                    .gap(px(8.))
                    .cursor_pointer()
                    .child(Self::checkbox_box(on))
                    .child(crate::assets::icon(icon, 20., cx))
                    .child(div().text_size(px(14.)).text_color(pal.text).child(name.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if context == 3 {
                            this.toolbar_draft.toggle_drawing_shortcut(&id2, &crate::drawing_keys::DEFAULT_SHORTCUT_TOOLS);
                        } else if context == 2 {
                            this.asm_toolbar_draft.toggle_shortcut(&id2);
                        } else if context == 1 {
                            this.toolbar_draft.toggle_shortcut(&id2);
                        } else {
                            this.toolbar_draft.toggle_sketch_shortcut(&id2, &sc::default_sketch_shortcut_names());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        // Live: the ticked tools in list order (a newly ticked tool takes its list place).
        // Live (z3): every ticked tool in list order, rows of five; no cap.
        let preview_icons: Vec<SharedString> = items.iter().filter(|(i, _, _)| shown.contains(i)).map(|(_, _, icon)| icon.clone()).collect();
        let preview = div()
            .ml(px(23.))
            .p(px(4.))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(pal.border)
            .shadow_sm()
            .when(preview_icons.is_empty(), |d| d.child(div().px(px(6.)).py(px(8.)).text_size(px(13.)).italic().text_color(rgb(0x999999)).child("No tools added")))
            .child(v_flex().children(preview_icons.chunks(5).map(|row| h_flex().children(row.iter().map(|icon| div().size(px(34.)).flex().items_center().justify_center().child(crate::assets::icon(icon, 20., cx))).collect::<Vec<_>>()))));
        Self::heading("Shortcut toolbars", None)
            .mt(px(38.))
            .child(div().mt(px(17.)).pl(px(16.)).child(select))
            .child(
                h_flex()
                    .mt(px(7.))
                    .pl(px(15.))
                    .items_start()
                    .child(div().id("tb-list").w(px(327.)).h(px(204.)).border_1().border_color(pal.border).overflow_y_scroll().child(v_flex().py(px(4.)).children(rows)))
                    .child(preview),
            )
            .child(h_flex().mt(px(12.)).pl(px(15.)).child(Self::save_button("tb-save", "Save shortcut toolbar settings", changed).when(changed, |b| {
                b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    // Keep the toolbar customizations saved meanwhile (Customize toolbar…).
                    let mut saved = ToolbarPrefs::load();
                    saved.shortcut_tools = this.toolbar_draft.shortcut_tools.clone();
                    saved.sketch_shortcut_tools = this.toolbar_draft.sketch_shortcut_tools.clone();
                    saved.drawing_shortcut_tools = this.toolbar_draft.drawing_shortcut_tools.clone();
                    saved.save();
                    this.toolbar = saved;
                    // The assembly's S toolbar (its other toolbar settings kept).
                    let mut asm = crate::assembly_view::AsmToolbarPrefs::load();
                    asm.shortcut_tools = this.asm_toolbar_draft.shortcut_tools.clone();
                    asm.save();
                    this.asm_toolbar = asm;
                    cx.notify();
                }))
            })))
            // Toolbars (live prefs/04): "Reset to defaults" puts the Part Studio
            // and Assembly toolbars back (tool sets, removed tools, Search tools).
            .child(
                Self::heading("Toolbars", None).mt(px(38.)).child(
                    h_flex().mt(px(16.)).pl(px(15.)).child(Self::save_button("tb-reset", "Reset to defaults", true).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        let mut ps = ToolbarPrefs::load();
                        ps.removed.clear();
                        ps.group_order.clear();
                        ps.hide_search = false;
                        ps.save();
                        this.toolbar = ps;
                        // The assembly toolbar (assembly_toolbar.rs): tool sets, removed
                        // tools and Search tools back; its S toolbar kept, like the Part Studio's.
                        let mut asm = crate::assembly_view::AsmToolbarPrefs::load();
                        asm.order.clear();
                        asm.removed.clear();
                        asm.hide_search = false;
                        asm.save();
                        window.refresh();
                        cx.notify();
                    }))),
                ),
            )
            // Live drawing/482: 15 px to the Drawings heading.
            .child(div().h(px(15.)))
            .into_any_element()
    }

    fn checkbox_box(on: bool) -> Div {
        div()
            .size(px(13.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.))
            .border_1()
            .border_color(if on { rgb(0x0075ff) } else { rgb(0x767676) })
            .bg(if on { rgb(0x0075ff) } else { rgb(0xffffff) })
            .when(on, |d| d.child(Icon::new(IconName::Check).size(px(11.)).text_color(rgb(0xffffff))))
    }

    fn render_profile(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let value = self.name.read(cx).value().trim().to_string();
        let changed = value != self.name_saved.trim();
        let os_name = noshape_store::os_user_name();
        Self::heading("Profile", None)
            .child(
                v_flex()
                    .mt(px(14.))
                    .pl(px(15.))
                    .w(px(460.))
                    .gap(px(6.))
                    .child(os::field_label("Name"))
                    .child(os::input_h(Input::new(&self.name), 32.).text_size(px(14.)))
                    .child(div().text_size(px(12.)).text_color(pal.text_muted).child(format!("Shown as the author in Versions and history. Leave empty to use your account name ({os_name})."))),
            )
            .child(h_flex().mt(px(25.)).pl(px(15.)).child(Self::save_button("profile-save", "Save", changed).when(changed, |b| {
                b.on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let v = this.name.read(cx).value().to_string();
                    crate::files::set_user_name_pref(&v);
                    this.name_saved = v.trim().to_string();
                    crate::start_window::refresh(cx);
                    window.refresh();
                    cx.notify();
                }))
            })))
            .into_any_element()
    }
}

/// The ✕ of the edit popup.
const CROSS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="4"><path d="m2.9 2.9 14.2 14.2m0-14.2L2.9 17.1"/></svg>"#;
/// The context menu key (≣) of the invalid keys list.
const MENU_KEY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M1.25 1.25h17.5V20H1.25zm1.25 1.25v16.25h15V2.5z"/><path d="M5 6.25h10V7.5H5zM5 10h10v1.25H5zm0 3.75h10V15H5z"/></svg>"#;

/// Bold ranges for the conflict alert's action names.
fn bold_names(name: &str) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let text = format!("This combination is already in use by {name}. Confirming the changes will disable the shortcut {name}.");
    let bold = HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() };
    text.match_indices(name).map(|(i, m)| (i..i + m.len(), bold)).collect()
}

/// The ⓘ popover: "The following keys are not valid entries:".
fn invalid_keys_popover(cx: &App) -> AnyElement {
    let pal = p();
    let chip = |t: &str| key_chip(t.to_string(), KeyStyle::Default).into_any_element();
    let menu = div().h(px(26.)).px(px(5.)).flex().items_center().border_1().border_b_2().border_color(rgb(CHIP_BORDER)).rounded(px(2.)).child(os::glyph(MENU_KEY, 16., pal.text)).into_any_element();
    let cell = |c: AnyElement| div().w(px(118.)).flex().justify_center().child(c);
    let pairs: Vec<(AnyElement, AnyElement)> = vec![(chip("Win"), chip("num lock")), (chip("backspace"), chip("← ↑ → ↓")), (chip("caps lock"), chip("scroll lock")), (chip("f1-12"), menu)];
    let _ = cx;
    div()
        .absolute()
        .left(px(28.))
        .top(px(-94.))
        .w(px(272.))
        .py(px(12.))
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(0xcccccc))
        .rounded(px(4.))
        .shadow_md()
        .child(div().px(px(14.)).text_size(px(13.33)).text_color(pal.text).child("The following keys are not valid entries:"))
        .child(v_flex().mt(px(6.)).gap(px(8.)).children(pairs.into_iter().map(|(a, b)| h_flex().child(cell(a)).child(cell(b)))))
        .into_any_element()
}

/// A key chip (`span.key`): 26 px, 1 px outline with a 2 px bottom, 14 px
/// text. Customized: blue outline; disabled: gray struck-through text.
pub fn key_chip(text: impl Into<SharedString>, style: KeyStyle) -> Div {
    let pal = p();
    let border = if style == KeyStyle::Customized { pal.accent } else { rgb(CHIP_BORDER) };
    div()
        .h(px(26.))
        .min_w(px(20.))
        .flex_none()
        .px(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_b_2()
        .border_color(border)
        .rounded(px(2.))
        .bg(rgb(0xffffff))
        .text_size(px(14.))
        .text_color(if style == KeyStyle::Disabled { pal.text_faint } else { pal.text })
        .when(style == KeyStyle::Disabled, |d| d.line_through())
        .child(text.into())
}

impl Render for PreferencesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        crate::assets::set_scale_factor(window.scale_factor());
        if self.scroll_to_shortcuts {
            // The sections are the scroll area's children: Keyboard shortcuts is the sixth.
            match self.scroll.bounds_for_item(5) {
                Some(section) if self.page != Page::Profile => {
                    let off = self.scroll.offset();
                    let top = section.top() - self.scroll.bounds().top() - off.y;
                    let y = top.min(self.scroll.max_offset().y).max(px(0.));
                    self.scroll.set_offset(point(off.x, -y));
                    self.scroll_to_shortcuts = false;
                }
                _ => window.request_animation_frame(),
            }
        }
        if self.scroll_to_end {
            let max = self.scroll.max_offset().y;
            if max > px(0.) {
                self.scroll.set_offset(point(px(0.), -max));
                self.scroll_to_end = false;
            } else {
                window.request_animation_frame();
            }
        }
        let content: Vec<AnyElement> = if self.page == Page::Profile {
            vec![self.render_profile(cx)]
        } else {
            let (ps, asm) = self.render_list_settings(cx);
            vec![self.decimal.clone().into_any_element(), self.units.clone().into_any_element(), self.mouse.clone().into_any_element(), ps, asm, self.render_shortcuts(window, cx), self.render_toolbars(cx), self.drawings.clone().into_any_element(), self.release.clone().into_any_element()]
        };
        v_flex()
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(pal.text)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .child(self.render_title_bar(cx))
            .child(self.render_header())
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.render_nav(cx))
                    .child(
                        div()
                            .id("prefs-content")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .pl(px(CONTENT_PAD))
                            .pr(px(55.))
                            .pt(px(34.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .children(content)
                            .vertical_scrollbar(&self.scroll),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn list_prefs_round_trip() {
        let p = ListPrefs { parts_list_properties: true, instance_list_properties: false, ..Default::default() };
        let text = serde_json::to_string(&p).unwrap_or_default();
        assert!(text.contains("partsListProperties"));
        assert_eq!(serde_json::from_str::<ListPrefs>(&text).ok(), Some(p));
        assert_eq!(serde_json::from_str::<ListPrefs>("{}").ok(), Some(ListPrefs::default()));
    }

    #[test]
    fn conflict_alert_bolds_both_names() {
        let ranges = bold_names("Line*");
        assert_eq!(ranges.len(), 2);
    }
}
