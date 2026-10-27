//! Preferences › Decimal format and Units (live prefs/00–01,
//! dom/preferences-full.txt): "Use comma as decimal separator"
//! (`noshape_units::set_comma_decimal`: typed expressions accept `1,5`, values
//! show with a comma), the default units of newly created documents, each
//! quantity's "Display decimals", and the Time format. Kept in the config
//! dir; new documents (`new_document`) start with these units, and document
//! times follow the time format (`start_library::format_onshape_time`).

pub use crate::user_units::{UserUnits, apply, current, new_document, save, time_24h};
use crate::user_units::{DECIMALS, OTHER_DECIMALS};
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_units::{AngleUnit, LENGTH_UNITS_SHOWN, OTHER_UNITS, OTHER_UNITS_SHOWN};
use std::cell::Cell;
use std::rc::Rc;

/// Unit select 226 px, decimals select 150 px, both 34 px (live 916–950).
const UNIT_W: f32 = 226.;
const DEC_W: f32 = 150.;
/// Live: Length 7 decimals options here (the Workspace units dialog has 6).
const LENGTH_DECIMALS: usize = 7;
const ANGLE_DECIMALS: usize = 7;

/// How a pick (the option's index) changes the draft.
type SetFn = Rc<dyn Fn(&mut UserUnits, usize)>;

/// One select of the section: its options, the shown index and how a pick
/// changes the draft.
struct Choice {
    id: &'static str,
    width: f32,
    options: Vec<String>,
    at: usize,
    set: SetFn,
}

pub struct UnitsPrefsView {
    saved: UserUnits,
    edited: UserUnits,
    /// The select whose native list is open (its id) and the list's
    /// dismiss subscription.
    open: Option<(&'static str, Subscription)>,
    /// When a list last closed: the click on its field that closed it must
    /// not reopen it.
    closed_at: Option<std::time::Instant>,
    bounds: std::collections::HashMap<&'static str, Rc<Cell<Bounds<Pixels>>>>,
}

impl Default for UnitsPrefsView {
    fn default() -> Self {
        Self::new()
    }
}

impl UnitsPrefsView {
    pub fn new() -> Self {
        let saved = current();
        Self { saved, edited: saved, open: None, closed_at: None, bounds: Default::default() }
    }

    fn decimals(n: usize) -> Vec<String> {
        DECIMALS[..n].iter().map(|s| s.to_string()).collect()
    }

    /// The rows in Onshape's order: (label, unit choice, decimals choice).
    fn rows(&self) -> Vec<(String, Choice, Choice)> {
        const IDS: [(&str, &str); 9] = [("u-0", "d-0"), ("u-1", "d-1"), ("u-2", "d-2"), ("u-3", "d-3"), ("u-4", "d-4"), ("u-5", "d-5"), ("u-6", "d-6"), ("u-7", "d-7"), ("u-8", "d-8")];
        let u = self.edited.units;
        let other = |k: usize| -> (String, Choice, Choice) {
            let (label, _, names, _, _) = OTHER_UNITS[k];
            let shown = OTHER_UNITS_SHOWN[k];
            let n = OTHER_DECIMALS[k];
            (
                format!("{label} default unit"),
                Choice { id: IDS[k].0, width: UNIT_W, options: shown.iter().map(|&i| names[i as usize].to_string()).collect(), at: shown.iter().position(|&i| i == u.other[k].0).unwrap_or(0), set: Rc::new(move |e, i| e.units.other[k].0 = shown[i]) },
                Choice { id: IDS[k].1, width: DEC_W, options: Self::decimals(n), at: (u.other[k].1 as usize).min(n - 1), set: Rc::new(move |e, i| e.units.other[k].1 = i as u8) },
            )
        };
        let length = (
            "Length default unit".to_string(),
            Choice { id: "u-length", width: UNIT_W, options: LENGTH_UNITS_SHOWN.iter().map(|l| l.name().to_string()).collect(), at: LENGTH_UNITS_SHOWN.iter().position(|l| *l == u.length).unwrap_or(0), set: Rc::new(|e, i| e.units.length = LENGTH_UNITS_SHOWN[i]) },
            Choice { id: "d-length", width: DEC_W, options: Self::decimals(LENGTH_DECIMALS), at: (u.length_precision as usize).min(LENGTH_DECIMALS - 1), set: Rc::new(|e, i| e.units.length_precision = i as u8) },
        );
        let angle = (
            "Angle default unit".to_string(),
            Choice { id: "u-angle", width: UNIT_W, options: AngleUnit::ALL.iter().map(|a| a.name().to_string()).collect(), at: AngleUnit::ALL.iter().position(|a| *a == u.angle).unwrap_or(0), set: Rc::new(|e, i| e.units.angle = AngleUnit::ALL[i]) },
            Choice { id: "d-angle", width: DEC_W, options: Self::decimals(ANGLE_DECIMALS), at: (u.angle_precision as usize).min(ANGLE_DECIMALS - 1), set: Rc::new(|e, i| e.units.angle_precision = i as u8) },
        );
        // Length, Linear acceleration, Angle, Angular velocity, then the mechanical quantities.
        let mut rows = vec![length, other(0), angle, other(1)];
        rows.extend((2..OTHER_UNITS.len()).map(other));
        rows
    }

    /// A browser `<select>` (live: 34 px, 1 px `#aaa`, radius 5.25, `#777` arrow) whose
    /// list is the native-select menu right under it.
    fn select(&mut self, c: Choice, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let bounds = self.bounds.entry(c.id).or_default().clone();
        let open = self.open.as_ref().is_some_and(|(id, _)| *id == c.id);
        let label = c.options.get(c.at).cloned().unwrap_or_default();
        let (id, width, options, at, set) = (c.id, c.width, c.options, c.at, c.set);
        let b2 = bounds.clone();
        div()
            .id(id)
            .relative()
            .w(px(width))
            .h(px(34.))
            .flex_none()
            .pl(px(10.))
            .pr(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(5.25))
            .border_1()
            .border_color(if open { rgb(0x1651b0) } else { rgb(0xaaaaaa) })
            .bg(rgb(0xffffff))
            .cursor_pointer()
            .text_size(px(14.))
            .text_color(pal.text)
            .child(div().flex_1().min_w_0().truncate().child(label))
            // Chrome's select arrow: an 8 × 5 `#777` triangle (live prefs/01).
            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., gpui_kit::rgb_static(0x777777)))
            .child(canvas(move |b, _, _| b2.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let was_open = this.open.take().is_some_and(|(o, _)| o == id);
                if was_open || this.closed_at.is_some_and(|t| t.elapsed().as_millis() < 300) {
                    cx.notify();
                    return;
                }
                let view = cx.entity();
                let (options, set) = (options.clone(), set.clone());
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    m = m.native_select(width);
                    for (i, o) in options.iter().enumerate() {
                        let (v, set) = (view.clone(), set.clone());
                        m = m.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(i == at).on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                set(&mut this.edited, i);
                                cx.notify();
                            })
                        }));
                    }
                    m
                });
                let sub = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
                    this.open = None;
                    this.closed_at = Some(std::time::Instant::now());
                    cx.notify();
                });
                this.open = Some((id, sub));
                let b = bounds.get();
                crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height + px(1.)), Anchor::TopLeft, menu, window, cx);
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for UnitsPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let label = |t: String| div().h(px(21.)).text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child(t);
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, (name, unit, dec)) in self.rows().into_iter().enumerate() {
            let (unit, dec) = (self.select(unit, cx), self.select(dec, cx));
            rows.push(
                h_flex()
                    .id(("units-row", i))
                    .items_end()
                    .mb(px(12.))
                    .child(v_flex().w(px(UNIT_W)).child(label(name)).child(div().mt(px(5.)).child(unit)))
                    .child(v_flex().ml(px(22.)).w(px(DEC_W)).child(label("Display decimals".into())).child(div().mt(px(5.)).child(dec)))
                    .child(div().ml(px(12.)).mb(px(7.)).size(px(20.)).child(crate::assets::icon("help-gray", 20., cx)))
                    .into_any_element(),
            );
        }
        let time = self.select(
            Choice { id: "u-time", width: DEC_W, options: vec!["12 hour".into(), "24 hour".into()], at: usize::from(self.edited.time_24h), set: Rc::new(|e, i| e.time_24h = i == 1) },
            cx,
        );
        v_flex()
            .mb(px(38.))
            .child(
                h_flex()
                    .h(px(32.))
                    .pl(px(15.))
                    .border_b_1()
                    .border_color(pal.text)
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Units")),
            )
            .child(
                v_flex()
                    .pl(px(15.))
                    .pt(px(18.))
                    .child(
                        h_flex()
                            // Live: 16 px (wider than the 14 px labels).
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .text_color(pal.text)
                            .child("Change the default units for all newly created documents. Units for existing documents are not changed.\u{a0}")
                            .child(
                                div()
                                    .id("units-learn-more")
                                    .text_color(crate::theme::p().link)
                                    .underline()
                                    .cursor_pointer()
                                    .child("Learn more about default units.")
                                    .on_click(|_, _, cx| cx.open_url("https://github.com/yekta/noshape#readme")),
                            ),
                    )
                    .child(v_flex().mt(px(20.)).children(rows))
                    .child(v_flex().child(label("Time format".into())).child(div().mt(px(5.)).child(time)))
                    .child(
                        h_flex().mt(px(24.)).child(
                            div()
                                .id("units-save")
                                .h(px(34.))
                                .px(px(10.5))
                                .flex()
                                .items_center()
                                .rounded(px(5.25))
                                .bg(gpui_kit::rgb_static(0x1651b0))
                                .text_size(px(14.))
                                .text_color(gpui_kit::rgb_static(0xffffff))
                                .when(!changed, |d| d.opacity(0.4))
                                .when(changed, |d| {
                                    d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        // The Decimal format section saves its own field.
                                        let mut u = current();
                                        u.units = this.edited.units;
                                        u.time_24h = this.edited.time_24h;
                                        save(u);
                                        this.saved = this.edited;
                                        // Document times shown elsewhere follow the new format.
                                        window.refresh();
                                        cx.notify();
                                    }))
                                })
                                .child("Save changes"),
                        ),
                    ),
            )
    }
}

/// Preferences › Decimal format (live prefs/00): the checkbox and "Save
/// decimal format".
pub struct DecimalPrefsView {
    saved: bool,
    edited: bool,
}

impl Default for DecimalPrefsView {
    fn default() -> Self {
        let saved = current().comma_decimal;
        Self { saved, edited: saved }
    }
}

impl Render for DecimalPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let (on, changed) = (self.edited, self.edited != self.saved);
        v_flex()
            .mb(px(38.))
            .child(
                h_flex()
                    .h(px(32.))
                    .pl(px(15.))
                    .border_b_1()
                    .border_color(pal.text)
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Decimal format")),
            )
            .child(
                v_flex()
                    .pl(px(15.))
                    .pt(px(16.))
                    .child(
                        // A browser checkbox: 13 px box, the label 3 px after it (live 686).
                        h_flex()
                            .id("decimal-comma")
                            .h(px(21.))
                            .gap(px(3.))
                            .cursor_pointer()
                            .text_size(px(14.))
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
                                    .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(gpui_kit::rgb_static(0xffffff)))),
                            )
                            .child("Use comma as decimal separator")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.edited = !this.edited;
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex().mt(px(30.)).child(
                            div()
                                .id("decimal-save")
                                .h(px(34.))
                                .px(px(10.5))
                                .flex()
                                .items_center()
                                .rounded(px(5.25))
                                .bg(gpui_kit::rgb_static(0x1651b0))
                                .text_size(px(14.))
                                .text_color(gpui_kit::rgb_static(0xffffff))
                                .when(!changed, |d| d.opacity(0.4))
                                .when(changed, |d| {
                                    d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        let mut u = current();
                                        u.comma_decimal = this.edited;
                                        save(u);
                                        noshape_units::set_comma_decimal(this.edited);
                                        this.saved = this.edited;
                                        // Values on screen follow the new separator.
                                        window.refresh();
                                        cx.notify();
                                    }))
                                })
                                .child("Save decimal format"),
                        ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::UnitPrefs;
    // `gpui_kit::*` (via super) exports gpui's `test` attribute; use std's.
    use std::prelude::v1::test;

    #[test]
    fn rows_follow_onshape() {
        let v = UnitsPrefsView::new();
        let rows = v.rows();
        let labels: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Length default unit",
                "Linear acceleration default unit",
                "Angle default unit",
                "Angular velocity default unit",
                "Mass default unit",
                "Density default unit",
                "Force default unit",
                "Frequency default unit",
                "Moment default unit",
                "Pressure default unit",
                "Energy default unit"
            ]
        );
        // Live's decimals counts: 7 7 7 7 8 9 9 9 8 9 9.
        let counts: Vec<usize> = rows.iter().map(|r| r.2.options.len()).collect();
        assert_eq!(counts, [7, 7, 7, 7, 8, 9, 9, 9, 8, 9, 9]);
    }

    #[test]
    fn prefs_round_trip() {
        let u = UserUnits { units: UnitPrefs::imperial(), time_24h: true, comma_decimal: true };
        let t = serde_json::to_string(&u).unwrap_or_default();
        assert_eq!(serde_json::from_str::<UserUnits>(&t).ok(), Some(u));
        assert_eq!(serde_json::from_str::<UserUnits>("{}").ok(), Some(UserUnits::default()));
    }
}
