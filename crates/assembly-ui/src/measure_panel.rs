//! Onshape's Measure panel (live part-studio/82, 190, 495; behaviours
//! "Measure & mass properties"): 220 px, docked at the bottom right, with
//! "Measure" and a close button, the "Select entities to measure" field,
//! Measure type, Length unit and Angle unit selects, the Reference coordinate
//! system checkbox, and one row per result (distances with signed X / Y / Z
//! sub-rows). Shared by the Assembly (icons2 Round 14: its panel lacked these
//! fields); the Part Studio has its own copy in part_studio/menus.rs with the
//! (x) variable buttons.

use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::regen::measure::Measurement;
use noshape_units::{AngleUnit, Dim, LengthUnit, Quantity, UnitPrefs};
use std::rc::Rc;

/// The Measure type select (bundle order).
pub const MEASURE_TYPES: [&str; 12] = ["Show all", "Position", "Center position", "Min distance", "Max distance", "Length", "Diameter", "Radius", "Angle", "Face tangent angle", "Curvature deviation", "Surface area"];

/// The Length unit choices (live order).
pub const LENGTH_UNITS: [(&str, LengthUnit); 6] = [
    ("Centimeter", LengthUnit::Centimeter),
    ("Foot", LengthUnit::Foot),
    ("Inch", LengthUnit::Inch),
    ("Meter", LengthUnit::Meter),
    ("Millimeter", LengthUnit::Millimeter),
    ("Yard", LengthUnit::Yard),
];

const ANGLE_UNITS: [(&str, AngleUnit); 2] = [("Degree", AngleUnit::Degree), ("Radian", AngleUnit::Radian)];

/// The panel's own choices: Measure type (None: Show all) and units (None:
/// the document's).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeasureSettings {
    pub kind: Option<&'static str>,
    pub length: Option<LengthUnit>,
    pub angle: Option<AngleUnit>,
}

impl MeasureSettings {
    pub fn units(&self, doc: &UnitPrefs) -> UnitPrefs {
        UnitPrefs { length: self.length.unwrap_or(doc.length), angle: self.angle.unwrap_or(doc.angle), ..*doc }
    }
}

/// One result row: label, value ("12.500 mm") and, for an X / Y / Z
/// sub-row, its axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub axis: Option<usize>,
}

/// The Measure type a row belongs to (bundle: each type and the rows it
/// keeps; the other distances only show with Show all).
pub fn filter_type(label: &str) -> Option<&'static str> {
    Some(match label {
        "Point" => "Position",
        "Center" => "Center position",
        "Min dist" => "Min distance",
        "Max dist" => "Max distance",
        "Length" => "Length",
        "Diameter" => "Diameter",
        "Radius" => "Radius",
        "Angle" | "Perp planes angle" | "Perp lines angle" | "Perp axes angle" | "Perp line and plane angle" => "Angle",
        "Curvature deviation" => "Curvature deviation",
        "Area" => "Surface area",
        _ => return None,
    })
}

/// A value as Onshape's panel writes it (bundle `formatValue`: "90.000 deg",
/// "11.902 in²"): the display precision with its trailing zeros.
pub fn format(q: Quantity, units: &UnitPrefs) -> String {
    let angle = q.dim == Dim::ANGLE;
    let decimals = if angle { units.angle_precision } else { units.length_precision };
    let mut num = noshape_units::to_fixed(q.value / units.si_factor(q.dim), decimals);
    if num.starts_with('-') && num[1..].chars().all(|c| c == '0' || c == '.') {
        num.remove(0);
    }
    let unit = if angle {
        units.angle.symbol().to_string()
    } else {
        let power = match q.dim.length {
            2 => "²",
            3 => "³",
            _ => "",
        };
        format!("{}{power}", units.length.symbol())
    };
    format!("{num} {unit}")
}

/// The rows for `results` with the panel's type filter and units.
pub fn rows(results: &[(String, Measurement)], settings: &MeasureSettings, doc_units: &UnitPrefs) -> Vec<Row> {
    let units = settings.units(doc_units);
    let f = |q: Quantity| format(q, &units);
    let l = |x: f64| f(Quantity::new(x, Dim::LENGTH));
    let mut out = Vec::new();
    // Live (part-studio/495): Min dist is listed before Max dist.
    let rank = |l: &str| match l {
        "Max dist" => 1,
        _ => 0,
    };
    let mut ordered: Vec<&(String, Measurement)> = results.iter().collect();
    ordered.sort_by_key(|(l, _)| rank(l));
    for (label, m) in ordered {
        if let Some(t) = settings.kind
            && filter_type(label) != Some(t)
        {
            continue;
        }
        let value = match m {
            Measurement::Length(x) | Measurement::Radius(x) | Measurement::Diameter(x) => l(*x),
            Measurement::Area(x) => f(Quantity::new(*x, Dim { length: 2, angle: 0 })),
            Measurement::Volume(x) => f(Quantity::new(*x, Dim { length: 3, angle: 0 })),
            Measurement::Angle(x) => f(Quantity::new(*x, Dim::ANGLE)),
            Measurement::Percent(x) => format!("{} %", noshape_units::to_fixed(*x, 2)),
            // Live: "Point: 0.000 in 0.197 in 0.787 in".
            Measurement::Point(p) => format!("{} {} {}", l(p.x), l(p.y), l(p.z)),
            Measurement::Distance { distance, .. } => l(*distance),
        };
        out.push(Row { label: label.clone(), value, axis: None });
        if let Measurement::Distance { dx, dy, dz, .. } = m {
            for (i, d) in [*dx, *dy, *dz].into_iter().enumerate() {
                out.push(Row { label: String::new(), value: l(d), axis: Some(i) });
            }
        }
    }
    out
}

type Set = Rc<dyn Fn(MeasureSettings, &mut App)>;
type Close = Rc<dyn Fn(&mut Window, &mut App)>;

/// The panel (positioned by the caller). `selected`: the picked entities'
/// names for the field; `set`: a choice changed; `close`: the ×.
pub fn panel(selected: Vec<String>, rows: Vec<Row>, settings: MeasureSettings, doc_units: UnitPrefs, set: Set, close: Close, cx: &App) -> Stateful<Div> {
    let pal = p();
    let units = settings.units(&doc_units);
    let plain = |cx: &App| ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.row_hover.into()).active(pal.accent_soft.into()).shadow(false);
    // A row's select: the value on an underline with a caret, its list below.
    let select = |id: &'static str, value: String, items: Vec<(String, MeasureSettings)>, cx: &App| {
        let set = set.clone();
        Button::new(id)
            .custom(plain(cx))
            .child(h_flex().w_full().child(div().flex_1().text_size(px(13.33)).child(value.clone())).child(crate::ui::glyph("ui-caret-down", 10., pal.text)))
            .flex_1()
            .h(px(22.))
            .px(px(4.))
            .rounded(px(0.))
            .border_b_1()
            .border_color(pal.border)
            .dropdown_menu(move |mut m, _, _| {
                for (name, next) in items.iter().cloned() {
                    let set = set.clone();
                    m = m.item(PopupMenuItem::new(name.clone()).checked(name == value).on_click(move |_, _, cx| set(next, cx)));
                }
                m
            })
            .into_any_element()
    };
    let current = settings.kind.unwrap_or("Show all");
    let type_items = MEASURE_TYPES.iter().map(|t| (t.to_string(), MeasureSettings { kind: (*t != "Show all").then_some(*t), ..settings })).collect();
    let length_items = LENGTH_UNITS.iter().map(|(n, u)| (n.to_string(), MeasureSettings { length: Some(*u), ..settings })).collect();
    let angle_items = ANGLE_UNITS.iter().map(|(n, u)| (n.to_string(), MeasureSettings { angle: Some(*u), ..settings })).collect();
    let length_name = LENGTH_UNITS.iter().find(|(_, u)| *u == units.length).map(|(n, _)| n.to_string()).unwrap_or_default();
    let angle_name = ANGLE_UNITS.iter().find(|(_, u)| *u == units.angle).map(|(n, _)| n.to_string()).unwrap_or_default();
    let select_row = |label: &'static str, menu: AnyElement| h_flex().h(px(26.)).px(px(5.)).gap(px(4.)).child(div().text_size(px(12.)).text_color(pal.text).child(label)).child(menu);
    // Onshape's selection box: `#def1fb` with a 1 px blue border (active),
    // the caption small on top once something is listed.
    let empty = selected.is_empty();
    let field = v_flex()
        .mx(px(3.))
        .my(px(2.))
        .rounded(px(2.))
        .border_1()
        .border_color(pal.accent)
        .bg(pal.row_hover)
        .child(div().px(px(6.)).flex().items_center().when(empty, |d| d.h(px(24.)).text_size(px(12.))).when(!empty, |d| d.h(px(16.)).pt(px(2.)).text_size(px(10.))).text_color(rgb(0x555555)).child("Select entities to measure"))
        .children(selected.into_iter().map(|t| h_flex().h(px(24.)).px(px(6.)).text_size(px(13.33)).text_color(rgb(0x555555)).child(div().flex_1().truncate().child(t)).child(div().text_color(pal.text_muted).child("×"))));
    // The distance row an X / Y / Z row belongs to (picks its glyph).
    let mut parent = String::new();
    let result_rows: Vec<AnyElement> = rows
        .into_iter()
        .map(|r| {
            if r.axis.is_none() {
                parent = r.label.clone();
            }
            // "12.5 in" -> monospace number on an underline and the unit.
            let (num, unit) = r.value.split_once(' ').map(|(n, u)| (n.to_string(), u.to_string())).unwrap_or((r.value.clone(), String::new()));
            let value = h_flex().gap(px(2.)).child(crate::ui::measure_value(num)).child(div().text_size(px(12.)).child(unit));
            let row = h_flex().min_h(px(26.)).px(px(4.)).gap(px(4.)).text_size(px(12.)).text_color(pal.text);
            match r.axis {
                // Live (part-studio/495): the letter, then a glyph in the axis
                // colour: arrows under a distance, converging arrows under Min
                // dist, an I-beam under Max dist.
                Some(i) => {
                    let (name, color, arrow) = [("X", 0xbd3039, "axis-arrow-x"), ("Y", 0x009400, "axis-arrow-y"), ("Z", 0x1651b0, "axis-arrow-z")][i];
                    let glyph = if parent.starts_with("Min") {
                        crate::ui::glyph("measure-min-dist", 16., rgb(color))
                    } else if parent.starts_with("Max") {
                        crate::ui::glyph("measure-max-dist", 16., rgb(color))
                    } else {
                        crate::ui::glyph(arrow, 14., rgb(color))
                    };
                    row.pl(px(20.)).child(h_flex().child(div().text_size(px(15.)).text_color(rgb(0x999999)).child(name)).child(glyph)).child(value)
                }
                None => row.child(div().child(format!("{}:", r.label))).child(value),
            }
            .into_any_element()
        })
        .collect();
    v_flex()
        .id("measure-panel")
        .occlude()
        .w(px(220.))
        .bg(pal.dialog_bg)
        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
        .text_color(pal.text)
        .child(
            h_flex().h(px(29.)).pl(px(6.)).border_b_1().border_color(pal.border).child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Measure")).child(
                div()
                    .id("measure-close")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor(CursorStyle::Arrow)
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("dialog-cancel", 24., cx))
                    .on_click(move |_, window, cx| close(window, cx)),
            ),
        )
        .child(field)
        .child(select_row("Measure type:", select("measure-type", current.to_string(), type_items, cx)))
        .child(select_row("Length unit:", select("measure-length-unit", length_name, length_items, cx)))
        .child(select_row("Angle unit:", select("measure-angle-unit", angle_name, angle_items, cx)))
        .child(crate::ui::os_checkbox("measure-ref", "Reference coordinate system", false))
        .children(result_rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn rows_filter_and_convert() {
        let doc = UnitPrefs::default();
        let results = vec![("Min dist".to_string(), Measurement::Distance { distance: 0.01, dx: 0.01, dy: 0.0, dz: 0.0, a: Default::default(), b: Default::default() }), ("Area".to_string(), Measurement::Area(1e-4))];
        let all = rows(&results, &MeasureSettings::default(), &doc);
        assert_eq!(all.len(), 5, "distance + X/Y/Z + area");
        let areas = rows(&results, &MeasureSettings { kind: Some("Surface area"), ..Default::default() }, &doc);
        assert_eq!(areas.len(), 1);
        let mm = rows(&results, &MeasureSettings { length: Some(LengthUnit::Millimeter), ..Default::default() }, &doc);
        assert!(mm[0].value.ends_with("mm"), "{:?}", mm[0]);
        let both = vec![("Max dist".to_string(), Measurement::Distance { distance: 0.02, dx: 0.0, dy: 0.0, dz: 0.02, a: Default::default(), b: Default::default() }), results[0].clone()];
        let r = rows(&both, &MeasureSettings::default(), &doc);
        assert_eq!((r[0].label.as_str(), r[4].label.as_str()), ("Min dist", "Max dist"));
    }
}
