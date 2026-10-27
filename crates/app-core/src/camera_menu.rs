//! Onshape's "Camera and render options" menu (the ▾ under the view cube),
//! shared by the Part Studio and the assembly. Live (dom/menu-camera-render,
//! behaviours.md "qa5 assembly camera menu"): Isometric, Dimetric, Trimetric |
//! ⚙ Graphics preferences… | Named views…, Previous view | Zoom to fit, Zoom
//! to selection (with a selection), Zoom to window | Perspective view (Part
//! Studio: Orient normal to sketch on edit) | the display mode, hidden edges
//! and tangent edges, each a ✓ submenu showing the current choice | View in
//! high quality (Part Studio: | Highlight boundary edges) | Section view….
//! Onshape's own list (bundle `viewCubeItems`) sets which rows show: no
//! hidden-edges row in Shaded without edges or Translucent, no tangent row
//! in Shaded without edges; "Disable high quality" while on; with a section
//! on, "Edit section view…" (unless its dialog is open) and "Turn off
//! section view". The host keeps the state and handles each [`Action`].

use crate::scene_builder::{DisplayMode, TangentEdges};
use gpui_kit::*;
use noshape_render::StandardView;

/// What the menu shows.
#[derive(Clone, Copy)]
pub struct State {
    pub perspective: bool,
    /// "Previous view" is enabled.
    pub has_history: bool,
    /// "Zoom to selection" is listed.
    pub has_selection: bool,
    pub mode: DisplayMode,
    pub hidden_edges: bool,
    pub tangent: TangentEdges,
    pub high_quality: bool,
    /// A section is on: the last item turns it off.
    pub section_on: bool,
    /// The Section view dialog is open.
    pub section_dialog: bool,
    /// The Section view item is listed (live psb 10-21 f: not while a
    /// feature dialog is open).
    pub show_section: bool,
    /// Part Studio only: "Orient normal to sketch on edit" and its state.
    pub orient: Option<bool>,
    /// Part Studio only: "Highlight boundary edges" and its state.
    pub boundary: Option<bool>,
}

/// What the user picked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    View(StandardView),
    GraphicsPrefs,
    NamedViews,
    PreviousView,
    ZoomFit,
    ZoomWindow,
    ZoomSelection,
    Perspective,
    Orient,
    Display(DisplayMode),
    HiddenEdges(bool),
    Tangent(TangentEdges),
    HighQuality,
    Boundary,
    Section,
    /// "Edit section view…": reopen the dialog on the current section.
    EditSection,
}

/// The display mode's row label.
pub fn display_label(m: DisplayMode) -> &'static str {
    match m {
        DisplayMode::ShadedWithEdges => "Shaded with edges",
        DisplayMode::Shaded => "Shaded without edges",
        DisplayMode::HiddenLine => "Unshaded",
        DisplayMode::Translucent => "Translucent",
    }
}

fn hidden_label(on: bool) -> &'static str {
    if on { "Hidden edges visible" } else { "Hidden edges removed" }
}

/// The menu's items into `m` (an [`crate::os_menu::OsMenu`] in camera style);
/// `on` runs each pick on `view`.
pub fn build<V: 'static>(mut m: crate::os_menu::OsMenu, s: State, view: &Entity<V>, on: fn(&mut V, Action, &mut Window, &mut Context<V>), window: &mut Window, cx: &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu {
    use crate::os_menu::OsMenuItem;
    let item = |label: &str, a: Action| {
        let v = view.clone();
        OsMenuItem::new(label.to_string()).on_click(move |_, w, cx| v.update(cx, |this, cx| on(this, a, w, cx)))
    };
    for (label, sv) in [("Isometric", StandardView::Isometric), ("Dimetric", StandardView::Dimetric), ("Trimetric", StandardView::Trimetric)] {
        m = m.item(item(label, Action::View(sv)));
    }
    m = m.separator().item(item("Graphics preferences…", Action::GraphicsPrefs).icon_name("settings"));
    m = m.separator().item(item("Named views…", Action::NamedViews)).item(item("Previous view", Action::PreviousView).disabled(!s.has_history));
    m = m.separator().item(item("Zoom to fit", Action::ZoomFit));
    if s.has_selection {
        m = m.item(item("Zoom to selection", Action::ZoomSelection));
    }
    m = m.item(item("Zoom to window", Action::ZoomWindow));
    m = m.separator().item(item("Perspective view", Action::Perspective).checked(s.perspective));
    if let Some(orient) = s.orient {
        m = m.item(item("Orient normal to sketch on edit", Action::Orient).checked(orient));
    }
    m = m.separator();
    // Live (analysis/01): Onshape's 11x8 #666 check before each submenu row.
    let v = view.clone();
    m = m.submenu_with_icon(Some("menu-check"), display_label(s.mode), window, cx, move |mut sm, _, _| {
        for dm in [DisplayMode::ShadedWithEdges, DisplayMode::Shaded, DisplayMode::HiddenLine, DisplayMode::Translucent] {
            let v = v.clone();
            sm = sm.item(OsMenuItem::new(display_label(dm)).checked(dm == s.mode).on_click(move |_, w, cx| v.update(cx, |this, cx| on(this, Action::Display(dm), w, cx))));
        }
        sm
    });
    let v = view.clone();
    let hidden_row = !matches!(s.mode, DisplayMode::Shaded | DisplayMode::Translucent);
    if hidden_row {
        m = m.submenu_with_icon(Some("menu-check"), hidden_label(s.hidden_edges), window, cx, move |mut sm, _, _| {
        for visible in [true, false] {
            let v = v.clone();
            sm = sm.item(OsMenuItem::new(hidden_label(visible)).checked(visible == s.hidden_edges).on_click(move |_, w, cx| v.update(cx, |this, cx| on(this, Action::HiddenEdges(visible), w, cx))));
        }
        sm
        });
    }
    let v = view.clone();
    if s.mode != DisplayMode::Shaded {
        m = m.submenu_with_icon(Some("menu-check"), s.tangent.label(), window, cx, move |mut sm, _, _| {
        for t in TangentEdges::ALL {
            let v = v.clone();
            sm = sm.item(OsMenuItem::new(t.label()).checked(t == s.tangent).on_click(move |_, w, cx| v.update(cx, |this, cx| on(this, Action::Tangent(t), w, cx))));
        }
        sm
        });
    }
    m = m.separator().item(item(if s.high_quality { "Disable high quality" } else { "View in high quality" }, Action::HighQuality));
    // Live (dom/menu-camera-render): its own group.
    if let Some(boundary) = s.boundary {
        m = m.separator().item(item("Highlight boundary edges", Action::Boundary).checked(boundary));
    }
    if !s.show_section {
        return m;
    }
    m = m.separator();
    if !s.section_on {
        return m.item(item("Section view…", Action::Section).icon_name("menu-section-view"));
    }
    if !s.section_dialog {
        m = m.item(item("Edit section view…", Action::EditSection).icon_name("menu-section-view"));
    }
    let off = item("Turn off section view", Action::Section);
    m.item(if s.section_dialog { off.icon_name("menu-section-view") } else { off })
}
