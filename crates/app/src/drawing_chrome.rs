//! Shared look of the drawing tool dialogs' header buttons (live drawing/124,
//! 127, 180, 261, 468, 494): Onshape's drawing dialogs draw the ✕ in
//! `#dd5228` (the Part Studio's is `#bd3039`), and a disabled ✓ keeps its
//! white check on the pale green `#cae8ca`.

use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// The disabled ✓ cell's fill.
pub const OK_DISABLED: u32 = 0xcae8ca;

/// The ✕ of a drawing tool dialog's header.
pub fn cancel_x(cx: &App) -> AnyElement {
    crate::assets::icon("drawing-cancel", 17., cx)
}

/// The drawing tool dialogs' shadow (live dom drawing-detail-dialog:
/// `rgba(154, 165, 175, 0.7) 0 0 4px`).
pub fn dialog_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(208. / 360., 0.12, 0.645, 0.7), offset: point(px(0.), px(0.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }]
}

/// A [`DrCheckbox`] click handler, given the new value.
type CheckHandler = std::rc::Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// Onshape's drawing checkbox. In the floating dialogs and the view
/// properties panel (live drawing/20, 127, 710; their native input is
/// hidden): a 12 px see-through box with a 1 px `#ddd` border, radius 3,
/// 1 px above centre; checked `#1651b0` with a white check; the label 9 px
/// after it. [`DrCheckbox::native`] in the Drawing properties and styles
/// panels (live drawing/240: Chrome's own): 13 px, `#767676` / `#0075ff`,
/// the label 6 px after it. The same calls as the kit's `Checkbox`.
#[derive(IntoElement)]
pub struct DrCheckbox {
    id: ElementId,
    label: Option<SharedString>,
    /// The label is user content (a sketch's name): never translated.
    user: bool,
    native: bool,
    checked: bool,
    disabled: bool,
    tooltip: Option<SharedString>,
    on_click: Option<CheckHandler>,
}

impl DrCheckbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), label: None, user: false, native: false, checked: false, disabled: false, tooltip: None, on_click: None }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Chrome's native checkbox (the Drawing properties and styles panels).
    pub fn native(mut self) -> Self {
        self.native = true;
        self
    }

    /// The label is user content.
    pub fn user(mut self) -> Self {
        self.user = true;
        self
    }

    /// Greyed and inert.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// [`DrCheckbox::on_click`] (the kit's other name for it).
    pub fn on_change(self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click(f)
    }

    pub fn on_click(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for DrCheckbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let pal = crate::theme::p();
        let checked = self.checked;
        let handler = self.on_click.filter(|_| !self.disabled);
        let label = self.label.map(|l| if self.user { crate::ui::user_text(l).into_any_element() } else { l.into_any_element() });
        let custom_box = || {
            div()
                .relative()
                .top(px(-1.))
                .size(px(12.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(gpui_kit::rgb(if checked { 0x1651b0 } else { 0xdddddd }))
                // Unchecked, the box is see-through (live: the dialog's #fdfdfd inside).
                .when(checked, |d| d.bg(gpui_kit::rgb(0x1651b0)))
                .when(checked, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff))))
        };
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(if self.native { 6. } else { 9. }))
            .text_size(px(12.))
            .text_color(pal.text)
            .when(self.disabled, |d| d.opacity(0.5))
            .when(handler.is_some(), |d| d.cursor_pointer())
            .child(if self.native { crate::preferences_window::PreferencesView::checkbox_box(checked) } else { custom_box() })
            .children(label)
            .when_some(self.tooltip, |d, t| d.tooltip(crate::ui::tooltip(t)))
            .when_some(handler, |d, f| d.on_click(move |_, w, cx| f(&!checked, w, cx)))
    }
}
