//! A date and time field as the browser's `datetime-local` input (live's Versions and history filter, "Date
//! modified": live2 2405 (c)): "mm/dd/yyyy, --:-- --" in segments, the clicked one selected; digits fill it and move
//! on as Chrome's do (a month's "1" waits for a second digit, a "2" is February; a year takes up to six), arrows step
//! it, Backspace clears it, Tab goes to the next; a calendar button. Its value is a whole date and time or nothing;
//! partly filled, it shows pink.

use crate::theme::p;
use chrono::{Datelike as _, NaiveDate, NaiveDateTime, NaiveTime};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

const MONTH: usize = 0;
const DAY: usize = 1;
const YEAR: usize = 2;
const HOUR: usize = 3;
const MINUTE: usize = 4;
const PERIOD: usize = 5;

/// Each segment's placeholder and its separator after it.
const SEGMENTS: [(&str, &str); 6] = [("mm", "/"), ("dd", "/"), ("yyyy", ", "), ("--", ":"), ("--", " "), ("--", "")];

/// The field's segments and what is being typed (no UI: the field's model).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Segments {
    /// Month, day, year, hour (1–12), minute, PM (0 / 1).
    pub seg: [Option<u32>; 6],
    pub selected: Option<usize>,
    /// Digits typed into the selected segment so far.
    typed: String,
}

impl Segments {
    /// The whole date and time, if every segment is set and it is a real date.
    pub fn value(&self) -> Option<NaiveDateTime> {
        let [Some(m), Some(d), Some(y), Some(h), Some(min), Some(pm)] = self.seg else { return None };
        let date = NaiveDate::from_ymd_opt(i32::try_from(y).ok()?, m, d)?;
        let time = NaiveTime::from_hms_opt(h % 12 + 12 * pm, min, 0)?;
        Some(date.and_time(time))
    }

    /// Some segments set but no whole value (Chrome's invalid, pink; live's Search stays off).
    pub fn is_partial(&self) -> bool {
        self.seg.iter().any(Option::is_some) && self.value().is_none()
    }

    /// The input's value as the browser gives it: "2030-12-31T13:05", or "" until whole.
    pub fn text(&self) -> String {
        self.value().map(|v| v.format("%Y-%m-%dT%H:%M").to_string()).unwrap_or_default()
    }

    fn range(i: usize) -> (u32, u32) {
        match i {
            MONTH => (1, 12),
            DAY => (1, 31),
            YEAR => (1, 275_760),
            HOUR => (1, 12),
            MINUTE => (0, 59),
            _ => (0, 1),
        }
    }

    fn advance(&mut self) {
        self.typed.clear();
        if let Some(i) = self.selected {
            self.selected = Some((i + 1).min(PERIOD));
        }
    }

    /// A digit into the selected segment (Chrome's rules: it moves on once no further digit could follow).
    pub fn digit(&mut self, d: u32) {
        let Some(i) = self.selected.filter(|i| *i != PERIOD) else { return };
        let (min, max) = Self::range(i);
        let width = if i == YEAR { 6 } else { 2 };
        let next: u32 = format!("{}{d}", self.typed).parse().unwrap_or(d);
        if next > max && !self.typed.is_empty() {
            // Too big with the digits before: they stand, the digit starts the next segment.
            self.advance();
            self.digit(d);
            return;
        }
        self.typed.push(char::from_digit(d, 10).unwrap_or('0'));
        self.seg[i] = Some(next);
        if self.typed.len() >= width || next * 10 > max {
            if next < min {
                self.seg[i] = Some(min);
            }
            self.advance();
        }
    }

    /// Up / down: the next or previous value, wrapping (an empty one starts at today's).
    pub fn step(&mut self, up: bool, today: NaiveDate) {
        let Some(i) = self.selected else { return };
        let (min, max) = Self::range(i);
        let start = match i {
            MONTH => today.month(),
            DAY => today.day(),
            YEAR => today.year().max(1) as u32,
            _ => min,
        };
        self.seg[i] = Some(match self.seg[i] {
            None => start,
            Some(v) if up => if v >= max { min } else { v + 1 },
            Some(v) => if v <= min { max } else { v - 1 },
        });
        self.typed.clear();
    }

    pub fn set_date(&mut self, date: NaiveDate) {
        self.seg[MONTH] = Some(date.month());
        self.seg[DAY] = Some(date.day());
        self.seg[YEAR] = Some(date.year().max(1) as u32);
    }
}

pub struct DateTimeField {
    s: Segments,
    focus: FocusHandle,
    /// The calendar's month shown (year, month), while it is open.
    calendar: Option<(i32, u32)>,
}

pub enum DateTimeFieldEvent {
    Changed,
}

impl EventEmitter<DateTimeFieldEvent> for DateTimeField {}

impl Focusable for DateTimeField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DateTimeField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self { s: Segments::default(), focus: cx.focus_handle(), calendar: None }
    }

    pub fn value(&self) -> Option<NaiveDateTime> {
        self.s.value()
    }

    pub fn is_partial(&self) -> bool {
        self.s.is_partial()
    }

    /// The value in local time, as milliseconds (a history filter's bound).
    pub fn millis(&self) -> Option<i64> {
        use chrono::TimeZone as _;
        self.value().and_then(|v| chrono::Local.from_local_datetime(&v).single()).map(|t| t.timestamp_millis())
    }

    pub fn text(&self) -> String {
        self.s.text()
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.s = Segments::default();
        self.calendar = None;
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(DateTimeFieldEvent::Changed);
        cx.notify();
    }

    fn select(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.s.selected = Some(i);
        self.s.typed.clear();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let k = e.keystroke.key.as_str();
        let m = e.keystroke.modifiers;
        if m.control || m.platform || m.alt {
            return;
        }
        let s = &mut self.s;
        match k {
            "left" => {
                s.selected = Some(s.selected.map_or(0, |i| i.saturating_sub(1)));
                s.typed.clear();
            }
            "right" => {
                s.selected = Some(s.selected.map_or(0, |i| (i + 1).min(PERIOD)));
                s.typed.clear();
            }
            "up" | "down" => s.step(k == "up", chrono::Local::now().date_naive()),
            "backspace" | "delete" => {
                if let Some(i) = s.selected {
                    s.seg[i] = None;
                    s.typed.clear();
                }
            }
            // Tab walks the segments, then leaves the field.
            "tab" if !m.shift && s.selected.is_some_and(|i| i < PERIOD) => s.advance(),
            "tab" if m.shift && s.selected.is_some_and(|i| i > 0) => {
                s.selected = s.selected.map(|i| i - 1);
                s.typed.clear();
            }
            "tab" => return,
            "a" | "p" if s.selected == Some(PERIOD) => s.seg[PERIOD] = Some(u32::from(k == "p")),
            _ => match k.parse::<u32>() {
                Ok(d) if k.len() == 1 => s.digit(d),
                _ => return,
            },
        }
        cx.stop_propagation();
        self.changed(cx);
    }

    fn pick_day(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.s.set_date(date);
        self.calendar = None;
        self.changed(cx);
    }

    fn render_calendar(&self, (year, month): (i32, u32), cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default();
        let lead = first.weekday().num_days_from_sunday() as i64;
        let shift = |y: i32, m: u32, by: i32| {
            let n = y * 12 + m as i32 - 1 + by;
            (n.div_euclid(12), (n.rem_euclid(12) + 1) as u32)
        };
        let nav = |id: &'static str, glyph: &'static str, by: i32, cx: &mut Context<Self>| {
            div().id(id).px(px(6.)).cursor_pointer().child(glyph).on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                if let Some((y, m)) = t.calendar {
                    t.calendar = Some(shift(y, m, by));
                }
                cx.notify();
            }))
        };
        let mut grid = v_flex().gap(px(2.)).child(h_flex().children(["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"].map(|d| div().w(px(28.)).text_center().text_color(pal.text_muted).child(d))));
        let chosen = match self.s.seg {
            [Some(m), Some(d), Some(y), ..] => NaiveDate::from_ymd_opt(y as i32, m, d),
            _ => None,
        };
        for week in 0..6 {
            let mut row = h_flex();
            for wd in 0..7 {
                let date = first + chrono::Duration::days(week * 7 + wd - lead);
                let inside = date.month() == month;
                row = row.child(
                    div()
                        .id(SharedString::from(format!("dtf-day-{week}-{wd}")))
                        .w(px(28.))
                        .h(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .when(!inside, |d| d.text_color(pal.text_muted))
                        .when(chosen == Some(date), |d| d.bg(pal.accent).text_color(gpui_kit::rgb(0xffffff)))
                        .hover(|d| d.bg(pal.row_hover))
                        .child(date.day().to_string())
                        .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| t.pick_day(date, cx))),
                );
            }
            grid = grid.child(row);
        }
        v_flex()
            .id("dtf-calendar")
            .occlude()
            .p(px(8.))
            .gap(px(6.))
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(4.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_size(px(12.))
            .text_color(pal.text)
            .on_mouse_down_out(cx.listener(|t, _: &MouseDownEvent, _, cx| {
                t.calendar = None;
                cx.notify();
            }))
            .child(h_flex().justify_between().child(nav("dtf-prev", "‹", -1, cx)).child(first.format("%B %Y").to_string()).child(nav("dtf-next", "›", 1, cx)))
            .child(grid)
            .into_any_element()
    }
}

impl Render for DateTimeField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let focused = self.focus.is_focused(window);
        if !focused {
            self.s.typed.clear();
        }
        let invalid = self.is_partial();
        let mut segments = h_flex().flex_1().min_w_0().text_size(px(14.));
        for (i, (placeholder, sep)) in SEGMENTS.iter().enumerate() {
            let shown = match (i, self.s.seg[i]) {
                (PERIOD, Some(v)) => if v == 1 { "PM".to_string() } else { "AM".to_string() },
                (YEAR, Some(v)) => format!("{v:04}"),
                (_, Some(v)) => format!("{v:02}"),
                (_, None) => placeholder.to_string(),
            };
            let on = focused && self.s.selected == Some(i);
            segments = segments
                .child(
                    div()
                        .id(SharedString::from(format!("dtf-seg-{i}")))
                        .rounded(px(2.))
                        .when(on, |d| d.bg(gpui_kit::rgb(0x1a73e8)).text_color(gpui_kit::rgb(0xffffff)))
                        .when(!on && self.s.seg[i].is_none(), |d| d.text_color(pal.text_muted))
                        .child(shown)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |t, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            t.select(i, window, cx);
                        })),
                )
                .when(!sep.is_empty(), |d| d.child(div().text_color(pal.text_muted).child(*sep)));
        }
        let calendar = self.calendar.map(|m| self.render_calendar(m, cx));
        h_flex()
            .id("date-time-field")
            .key_context("DateTimeField")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .when(invalid, |d| d.bg(gpui_kit::rgb(0xfde7e9)))
            .on_key_down(cx.listener(Self::on_key))
            // A click past the segments selects the nearest end (the browser's input).
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _: &MouseDownEvent, window, cx| {
                let i = t.s.selected.unwrap_or(MONTH);
                t.select(i, window, cx);
            }))
            .child(segments)
            .child(
                div()
                    .id("dtf-calendar-button")
                    .pr(px(6.))
                    .cursor_pointer()
                    .child(crate::assets::icon("calendar", 16., cx))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        let today = chrono::Local::now().date_naive();
                        let shown = match t.s.seg {
                            [Some(m), _, Some(y), ..] => (y as i32, m),
                            _ => (today.year(), today.month()),
                        };
                        t.calendar = if t.calendar.is_some() { None } else { Some(shown) };
                        cx.notify();
                    })),
            )
            .children(calendar.map(|c| div().absolute().top(px(34.)).left_0().child(deferred(anchored().child(c)).with_priority(101))))
    }
}

#[cfg(test)]
mod tests {
    use super::{DAY, HOUR, MONTH, PERIOD, Segments, YEAR};
    use chrono::NaiveDate;

    fn typed(keys: &str, from: usize) -> Segments {
        let mut s = Segments { selected: Some(from), ..Default::default() };
        for d in keys.chars().filter_map(|c| c.to_digit(10)) {
            s.digit(d);
        }
        s
    }

    /// Chrome's datetime-local: each segment moves on once no further digit could follow; a whole value reads as
    /// the input's ("2030-12-31T13:05"), a part of one as "" (invalid).
    #[test]
    fn digits_fill_the_segments_as_the_browsers_do() {
        let mut s = typed("12312030", MONTH);
        assert_eq!((s.seg[YEAR], s.selected), (Some(2030), Some(YEAR)), "a year takes up to six digits: {s:?}");
        s.advance();
        for d in [0, 1, 0, 5] {
            s.digit(d);
        }
        assert_eq!(s.seg, [Some(12), Some(31), Some(2030), Some(1), Some(5), None], "{s:?}");
        assert!(s.is_partial());
        assert_eq!(s.text(), "");
        s.seg[PERIOD] = Some(1);
        assert_eq!(s.text(), "2030-12-31T13:05");
        // A month's "2" can't take another digit: February, on to the day.
        let s = typed("2", MONTH);
        assert_eq!((s.seg[MONTH], s.selected), (Some(2), Some(DAY)));
        // A "1" waits for a second.
        let s = typed("1", MONTH);
        assert_eq!((s.seg[MONTH], s.selected), (Some(1), Some(MONTH)));
        // live2 2405 (c): typed into the year, "12/31/2030" fills it with six digits, the rest goes on.
        let s = typed("12312030", YEAR);
        assert_eq!(s.seg[YEAR], Some(123_120), "{s:?}");
        assert_eq!(s.seg[HOUR], Some(3), "{s:?}");
        // Up on an empty month starts at today's; it wraps.
        let mut s = Segments { selected: Some(MONTH), ..Default::default() };
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        s.step(true, today);
        assert_eq!(s.seg[MONTH], Some(11));
        s.seg[MONTH] = Some(12);
        s.step(true, today);
        assert_eq!(s.seg[MONTH], Some(1));
    }
}
