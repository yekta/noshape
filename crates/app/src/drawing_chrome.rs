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

/// Onshape's drawing-dialog checkbox (live drawing/20, 710): a 12 px
/// see-through box with a 1 px `#ddd` border, radius 3, 1 px above centre;
/// checked `#1651b0` with a white check; the label 9 px after it. The same
/// calls as the kit's `Checkbox` (`label`, `checked`, `on_click` with the
/// new value).
#[derive(IntoElement)]
pub struct DrCheckbox {
    id: ElementId,
    label: Option<SharedString>,
    checked: bool,
    on_click: Option<CheckHandler>,
}

impl DrCheckbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), label: None, checked: false, on_click: None }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for DrCheckbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let pal = crate::theme::p();
        let (checked, handler) = (self.checked, self.on_click);
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(9.))
            .text_size(px(12.))
            .text_color(pal.text)
            .when(handler.is_some(), |d| d.cursor_pointer())
            .child(
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
                    .when(checked, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)))),
            )
            .children(self.label)
            .when_some(handler, |d, f| d.on_click(move |_, w, cx| f(&!checked, w, cx)))
    }
}
