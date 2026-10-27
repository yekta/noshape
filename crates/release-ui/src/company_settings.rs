//! Company settings (Onshape: user menu › Company settings; Help
//! enterprise_settings and its images in research ui2-help): a breadcrumb bar
//! "Company settings › <section>" over a left navigation and the section's
//! page. The sections are the Company plan's that work on this computer:
//! Numbering schemes, Release management and Revision tools (users, teams,
//! webhooks and the other cloud sections have no local counterpart).

use crate::actions::CloseWindow;
use crate::theme::p;
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// The left navigation's sections, in Onshape's order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    NumberingSchemes,
    ReleaseManagement,
    RevisionTools,
}

impl Section {
    const ALL: [Section; 3] = [Section::NumberingSchemes, Section::ReleaseManagement, Section::RevisionTools];

    fn label(self) -> &'static str {
        match self {
            Section::NumberingSchemes => "Numbering schemes",
            Section::ReleaseManagement => "Release management",
            Section::RevisionTools => "Revision tools",
        }
    }
}

/// Help image (enterprise settings › Revision tools, 1:1): the navigation is
/// 200 px with a `#dde5ec` edge, its rows 30 px of 15 px text.
const NAV_W: f32 = 200.;
const NAV_ROW: f32 = 30.;
const NAV_EDGE: u32 = 0xdde5ec;
/// The breadcrumb bar: 50 px of `#f5f5f5`.
const BAR_H: f32 = 50.;
const BAR_BG: u32 = 0xf5f5f5;

struct CompanyWindow {
    handle: AnyWindowHandle,
    view: WeakEntity<CompanySettingsView>,
}
impl Global for CompanyWindow {}

/// Opens Company settings at `section` (or brings the open page to the front there).
pub fn show(section: Section, cx: &mut App) {
    if let Some(g) = cx.try_global::<CompanyWindow>() {
        let (handle, view) = (g.handle, g.view.clone());
        let shown = handle.update(cx, |_, window, cx| {
            window.activate_window();
            let _ = view.update(cx, |v, cx| v.go_to(section, cx));
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
        window.set_window_title("Company settings — noshape");
        let view = cx.new(|cx| CompanySettingsView::new(section, window, cx));
        let fh = view.read(cx).focus.clone();
        window.defer(cx, move |window, cx| fh.focus(window, cx));
        view
    }) {
        Ok((handle, view)) => cx.set_global(CompanyWindow { handle, view: view.downgrade() }),
        Err(e) => log::error!("could not open company settings: {e:#}"),
    }
}

/// Called when a window closed: forgets the page if it was that one.
pub fn forget_if(id: WindowId, cx: &mut App) -> bool {
    let is_company = cx.try_global::<CompanyWindow>().is_some_and(|g| g.handle.window_id() == id);
    if is_company {
        cx.remove_global::<CompanyWindow>();
    }
    is_company
}

pub struct CompanySettingsView {
    section: Section,
    focus: FocusHandle,
    scroll: ScrollHandle,
    release: Entity<crate::release_prefs::ReleasePrefsView>,
    numbering: Entity<crate::numbering_prefs::NumberingPrefsView>,
    revision_tools: Entity<crate::revision_tools::RevisionToolsView>,
}

impl CompanySettingsView {
    fn new(section: Section, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            section,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            release: cx.new(|cx| crate::release_prefs::ReleasePrefsView::new(window, cx)),
            numbering: cx.new(|cx| crate::numbering_prefs::NumberingPrefsView::new(window, cx)),
            revision_tools: cx.new(|cx| crate::revision_tools::RevisionToolsView::new(window, cx)),
        }
    }

    pub fn go_to(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        cx.notify();
    }

    fn render_bar(&self) -> AnyElement {
        let pal = p();
        h_flex()
            .h(px(BAR_H))
            .flex_none()
            .px(px(10.))
            .gap(px(8.))
            .bg(rgb(BAR_BG))
            .text_size(px(18.))
            .child(div().text_color(pal.text_muted).child("Company settings"))
            .child(crate::ui::glyph("ui-chevron-right", 12., pal.text_muted))
            .child(div().font_weight(FontWeight::MEDIUM).text_color(pal.text).child(self.section.label()))
            .into_any_element()
    }

    fn render_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let rows = Section::ALL.into_iter().map(|s| {
            let active = s == self.section;
            h_flex()
                .id(SharedString::from(format!("company-nav-{}", s.label())))
                .h(px(NAV_ROW))
                .pl(px(20.))
                .relative()
                .text_size(px(15.))
                .cursor_pointer()
                .when(active, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(pal.accent).child(div().absolute().left_0().top_0().bottom_0().w(px(5.)).bg(pal.accent)))
                .when(!active, |d| d.text_color(pal.text).hover(|d| d.text_color(pal.accent)))
                .child(s.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.go_to(s, cx)))
        });
        v_flex()
            .w(px(NAV_W))
            .h_full()
            .flex_none()
            .pt(px(14.))
            .border_r_1()
            .border_color(rgb(NAV_EDGE))
            .children(rows)
            .into_any_element()
    }
}

impl Render for CompanySettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        crate::assets::set_scale_factor(window.scale_factor());
        let page: AnyElement = match self.section {
            Section::NumberingSchemes => self.numbering.clone().into_any_element(),
            Section::ReleaseManagement => self.release.clone().into_any_element(),
            Section::RevisionTools => self.revision_tools.clone().into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(pal.text)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .child(crate::preferences_window::account_title_bar(cx))
            .child(self.render_bar())
            .child(
                h_flex().flex_1().min_h_0().items_start().child(self.render_nav(cx)).child(
                    div()
                        .id("company-content")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        // Help image: the heading row 10 px under the bar.
                        .pt(px(10.))
                        .pl(px(15.))
                        .pr(px(30.))
                        .pb(px(40.))
                        .child(page),
                ),
            )
            // Success toasts ("Onshape Default revision table updated.").
            .relative()
            .children(crate::prefs_general::toast_overlay(cx))
    }
}
