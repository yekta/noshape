//! The Notifications bell (live, behaviours "dx (2026-10-28, P3) EX11" and
//! the import flow): a bell in the navbar with a blue unread count ("9+");
//! its panel "Notifications" (Only show unread, ✕) lists the newest first,
//! each a collapsed row (blue unread dot, ⌄, bold title, the time) that
//! opens to its message: "Export completed" / "<file> was exported
//! successfully.", a failure with a red ⓘ. The header's ⋯ (live request
//! 1735): Mark all as read · Expand all · Delete all. Kept for the app's
//! session.

use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub id: u64,
    pub title: String,
    pub body: String,
    /// Seconds since the epoch.
    pub time: i64,
    pub unread: bool,
    /// Open (its message shown).
    pub open: bool,
    /// A failure (red ⓘ).
    pub failed: bool,
}

/// The session's notifications, newest first.
#[derive(Default)]
pub struct Notifications {
    pub items: Vec<Notification>,
    pub only_unread: bool,
    next: u64,
}

impl Global for Notifications {}

/// Add one (and redraw the windows: the bell's count).
pub fn push(cx: &mut App, title: impl Into<String>, body: impl Into<String>, failed: bool) {
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let n = cx.default_global::<Notifications>();
    n.next += 1;
    let id = n.next;
    n.items.insert(0, Notification { id, title: title.into(), body: body.into(), time, unread: true, open: false, failed });
    cx.refresh_windows();
}

pub fn unread(cx: &App) -> usize {
    cx.try_global::<Notifications>().map_or(0, |n| n.items.iter().filter(|i| i.unread).count())
}

/// The bell's badge text: the unread count, "9+" past nine.
pub fn badge(count: usize) -> Option<String> {
    match count {
        0 => None,
        1..=9 => Some(count.to_string()),
        _ => Some("9+".into()),
    }
}

/// "9:22 AM Today", else "9:22 AM Oct 31" (local time).
pub fn time_label(secs: i64, now: i64) -> String {
    use chrono::{Local, TimeZone};
    let (Some(t), Some(n)) = (Local.timestamp_opt(secs, 0).single(), Local.timestamp_opt(now, 0).single()) else { return String::new() };
    let clock = t.format("%-I:%M %p").to_string();
    if t.date_naive() == n.date_naive() { format!("{clock} Today") } else { format!("{clock} {}", t.format("%b %-d")) }
}

fn update(cx: &mut App, f: impl FnOnce(&mut Notifications)) {
    f(cx.default_global::<Notifications>());
    cx.refresh_windows();
}

/// The panel under the bell: header, then the items (a click opens one and
/// marks it read).
pub fn panel(on_close: impl Fn(&mut Window, &mut App) + 'static, cx: &App) -> AnyElement {
    let pal = p();
    let blue = gpui_kit::rgb(0x1651b0);
    let (items, only_unread) = cx.try_global::<Notifications>().map_or((Vec::new(), false), |n| (n.items.clone(), n.only_unread));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let shown: Vec<Notification> = items.into_iter().filter(|i| !only_unread || i.unread).collect();
    let mut list = v_flex().id("notifications-list").flex_1().min_h_0().overflow_y_scroll();
    if shown.is_empty() {
        list = list.child(div().p(px(16.)).text_size(px(13.)).text_color(pal.text_muted).child(if only_unread { "No unread notifications." } else { "No notifications." }));
    }
    for n in shown {
        let id = n.id;
        list = list.child(
            v_flex()
                .id(SharedString::from(format!("notification-{id}")))
                .px(px(10.))
                .py(px(8.))
                .gap(px(4.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .cursor_pointer()
                .hover(|d| d.bg(pal.row_hover))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .child(div().size(px(7.)).flex_none().rounded_full().when(n.unread, |d| d.bg(blue)))
                        .child(crate::ui::glyph(if n.open { "ui-caret-down" } else { "ui-caret-right" }, 9., pal.text))
                        .when(n.failed, |d| d.child(div().size(px(14.)).flex_none().rounded_full().bg(gpui_kit::rgb(0xd9534f)).flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("i")))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(crate::ui::user_text(n.title.clone())))
                        .child(div().flex_none().text_size(px(11.)).text_color(pal.text_muted).child(time_label(n.time, now))),
                )
                // An import's steps (" · " apart) are rows with a green ✓ (live).
                .when(n.open && n.body.contains(" · "), |d| {
                    d.children(n.body.split(" · ").map(|step| {
                        h_flex().pl(px(28.)).gap(px(6.)).text_size(px(13.)).text_color(pal.text).child(div().text_color(gpui_kit::rgb(0x2e8b3a)).font_weight(FontWeight::BOLD).child("✓")).child(crate::ui::user_text(step.to_string()))
                    }))
                })
                .when(n.open && !n.body.contains(" · "), |d| d.child(div().pl(px(28.)).text_size(px(13.)).text_color(pal.text).child(crate::ui::user_text(n.body.clone()))))
                .on_click(move |_, _, cx| {
                    update(cx, |s| {
                        if let Some(i) = s.items.iter_mut().find(|i| i.id == id) {
                            i.open = !i.open;
                            i.unread = false;
                        }
                    })
                }),
        );
    }
    v_flex()
        .id("notifications-panel")
        .w(px(360.))
        .h(px(420.))
        .bg(pal.panel)
        .border_1()
        .border_color(gpui_kit::rgb(0xd7d7d7))
        .rounded(px(4.))
        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
        .occlude()
        .child(
            h_flex()
                .h(px(40.))
                .flex_none()
                .px(px(10.))
                .gap(px(8.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0xd7d7d7))
                .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Notifications"))
                .child(
                    h_flex()
                        .id("notifications-unread")
                        .gap(px(5.))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(pal.text)
                        .child(div().w(px(26.)).h(px(14.)).rounded_full().bg(if only_unread { blue } else { gpui_kit::rgb(0xbbbbbb) }).child(div().size(px(10.)).mt(px(2.)).rounded_full().bg(gpui_kit::rgb_static(0xffffff)).when(only_unread, |d| d.ml(px(14.))).when(!only_unread, |d| d.ml(px(2.)))))
                        .child("Only show unread")
                        .on_click(|_, _, cx| update(cx, |s| s.only_unread = !s.only_unread)),
                )
                .child(
                    div()
                        .id("notifications-more")
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .rounded(px(3.))
                        .hover(|d| d.bg(pal.row_hover))
                        .text_size(px(14.))
                        .text_color(pal.text_muted)
                        .child("⋯")
                        .on_click(|_, window, cx| {
                            let menu = crate::os_menu::OsMenu::build(window, cx, |m, _, _| {
                                let item = |label: &'static str, f: fn(&mut Notifications)| crate::os_menu::OsMenuItem::new(label).on_click(move |_, _, cx| update(cx, f));
                                m.dropdown()
                                    .item(item("Mark all as read", |s| s.items.iter_mut().for_each(|i| i.unread = false)))
                                    .item(item("Expand all", |s| s.items.iter_mut().for_each(|i| i.open = true)))
                                    .item(item("Delete all", |s| s.items.clear()))
                            });
                            crate::os_menu::open_anchored(window.mouse_position(), Anchor::TopRight, menu, window, cx);
                        }),
                )
                .child(div().id("notifications-close").size(px(20.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 12., pal.text_muted)).on_click(move |_, window, cx| on_close(window, cx))),
        )
        .child(list)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn badge_counts_up_to_nine_plus() {
        assert_eq!((super::badge(0), super::badge(3), super::badge(12)), (None, Some("3".into()), Some("9+".into())));
    }
}
