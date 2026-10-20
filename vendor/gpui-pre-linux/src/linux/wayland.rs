mod client;
mod clipboard;
mod cursor;
mod display;
mod popup;
mod serial;
mod window;

/// Contains Types for configuring layer_shell surfaces.
pub mod layer_shell;

pub(crate) use client::*;

use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

use gpui::CursorStyle;

/// NOSHAPE PATCH: the cursor-shape-v1 shape of a CSS standard cursor name.
pub(super) fn css_to_shape(css: &str) -> Option<Shape> {
    Some(match css {
        // Chrome shows all-scroll for `move` (Blink doesn't tell them apart).
        "move" => Shape::AllScroll,
        "zoom-in" => Shape::ZoomIn,
        "no-drop" => Shape::NoDrop,
        _ => return None,
    })
}

/// NOSHAPE PATCH: cursor theme names of a CSS standard cursor name.
pub(super) fn css_icon_names(css: &str) -> &'static [&'static str] {
    match css {
        "move" => &["all-scroll", "fleur", "size_all"],
        "zoom-in" => &["zoom-in"],
        "no-drop" => &["no-drop", "dnd-no-drop", "not-allowed", "crossed_circle"],
        _ => &["default"],
    }
}

pub(super) fn to_shape(style: CursorStyle) -> Shape {
    match style {
        CursorStyle::Arrow => Shape::Default,
        CursorStyle::IBeam => Shape::Text,
        CursorStyle::Crosshair => Shape::Crosshair,
        CursorStyle::ClosedHand => Shape::Grabbing,
        CursorStyle::OpenHand => Shape::Grab,
        CursorStyle::PointingHand => Shape::Pointer,
        CursorStyle::ResizeLeft => Shape::WResize,
        CursorStyle::ResizeRight => Shape::EResize,
        CursorStyle::ResizeLeftRight => Shape::EwResize,
        CursorStyle::ResizeUp => Shape::NResize,
        CursorStyle::ResizeDown => Shape::SResize,
        CursorStyle::ResizeUpDown => Shape::NsResize,
        CursorStyle::ResizeUpLeftDownRight => Shape::NwseResize,
        CursorStyle::ResizeUpRightDownLeft => Shape::NeswResize,
        CursorStyle::ResizeColumn => Shape::ColResize,
        CursorStyle::ResizeRow => Shape::RowResize,
        CursorStyle::IBeamCursorForVerticalLayout => Shape::VerticalText,
        CursorStyle::OperationNotAllowed => Shape::NotAllowed,
        CursorStyle::DragLink => Shape::Alias,
        CursorStyle::DragCopy => Shape::Copy,
        CursorStyle::ContextualMenu => Shape::ContextMenu,
    }
}
