//! Frame › Sketch profile › Libraries (live behaviours "psui (2026-11-01) Frame Sketch profile picker", D134): the
//! profile library's levels (Library › Standard or Type › Profile › Size) with live's lists and defaults, and each
//! profile's geometry built from the published dimensions its size names (no Onshape sketch is used). A Frame's
//! `profileSketch` names one as `frame-library:<standard>/<profile>`, its configuration after a tab
//! (`Size=…;Rounded=true`), as the select panel stores a configured pick.

use super::frames_profiles::{self as fp, FrameProfile};
use glam::DVec2;
use noshape_kernel as k;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// `profileSketch` of a library profile.
pub const PREFIX: &str = "frame-library:";

/// The library's one name (live: "Onshape frame profiles").
pub const LIBRARY: &str = "Onshape frame profiles";

/// The configuration inputs a library profile has.
pub const SIZE: &str = "Size";
pub const ROUNDED: &str = "Rounded";

#[derive(serde::Deserialize)]
struct SizeList {
    default: String,
    sizes: Vec<String>,
}

/// Live's size lists by standard and profile (dom/psui-d134-frame-profile-sizes-*.json).
static LISTS: LazyLock<BTreeMap<String, BTreeMap<String, SizeList>>> = LazyLock::new(|| serde_json::from_str(include_str!("frame_library.json")).unwrap_or_default());

/// A standard of the library, in the picker's order, and how its second level is labelled: ANSI and ISO have no Type
/// level, their select reads "Type" (live D134).
pub fn standards() -> Vec<&'static str> {
    LISTS.keys().map(|s| s.as_str()).collect()
}

/// The second level's label for `standard`.
pub fn standard_label(_standard: &str) -> &'static str {
    "Type"
}

/// A standard's profiles, in live's order.
pub fn profiles(standard: &str) -> Vec<&'static str> {
    const ORDER: [&str; 4] = ["Rectangular bar", "Rectangular tube", "Round bar", "Round tube"];
    let Some(s) = LISTS.get(standard) else { return Vec::new() };
    ORDER.iter().copied().filter(|p| s.contains_key(*p)).collect()
}

/// A profile's sizes and the default one.
pub fn sizes(standard: &str, profile: &str) -> (Vec<&'static str>, &'static str) {
    match LISTS.get(standard).and_then(|s| s.get(profile)) {
        Some(l) => (l.sizes.iter().map(|s| s.as_str()).collect(), l.default.as_str()),
        None => (Vec::new(), ""),
    }
}

/// Whether a profile has the Rounded checkbox (live: ISO's Rectangular tube).
pub fn has_rounded(standard: &str, profile: &str) -> bool {
    standard == "ISO" && profile == "Rectangular tube"
}

/// The `profileSketch` value of a library profile with its configuration.
pub fn value(standard: &str, profile: &str, size: &str, rounded: bool) -> String {
    let mut v = format!("{PREFIX}{standard}/{profile}\t{SIZE}={size}");
    if has_rounded(standard, profile) {
        v.push_str(&format!(";{ROUNDED}={rounded}"));
    }
    v
}

/// A library pick read back: (standard, profile, size, rounded); a missing size is the profile's default.
pub fn parse_value(value: &str) -> Option<(String, String, String, bool)> {
    let rest = value.strip_prefix(PREFIX)?;
    let (key, config) = rest.split_once('\t').unwrap_or((rest, ""));
    let (standard, profile) = key.split_once('/')?;
    let mut size = None;
    let mut rounded = false;
    for kv in config.split(';').filter(|s| !s.is_empty()) {
        match kv.split_once('=') {
            Some((SIZE, v)) => size = Some(v.to_string()),
            Some((ROUNDED, v)) => rounded = v == "true",
            _ => {}
        }
    }
    let size = size.unwrap_or_else(|| sizes(standard, profile).1.to_string());
    Some((standard.into(), profile.into(), size, rounded))
}

/// One length of a size name in millimetres: `0.375in`, `20mm`, `0.5"`, `1/32in`, `5 1/2in`, `0.25in DIA`; a bare
/// number is in inches (ANSI's "6in x 4in" spells them, its round bars' "(0.03125)" repeat them).
fn length_mm(token: &str) -> Option<f64> {
    let t = token.replace("DIA", "").replace("OD", "");
    let t = t.trim();
    let (num, mm) = if let Some(n) = t.strip_suffix("mm") {
        (n.trim(), true)
    } else {
        (t.trim_end_matches("in").trim_end_matches('"').trim(), false)
    };
    let mut v = 0.0;
    for part in num.split_whitespace() {
        v += match part.split_once('/') {
            Some((a, b)) => a.parse::<f64>().ok()? / b.parse::<f64>().ok()?,
            None => part.parse::<f64>().ok()?,
        };
    }
    Some(if mm { v } else { v * 25.4 })
}

/// A wall in Standard Wire Gauge (live's ISO round tubes: 10SWG, 16SWG, 18SWG), in millimetres.
fn swg_mm(token: &str) -> Option<f64> {
    let inches = match token.trim().strip_suffix("SWG")? {
        "10" => 0.128,
        "12" => 0.104,
        "14" => 0.080,
        "16" => 0.064,
        "18" => 0.048,
        "20" => 0.036,
        _ => return None,
    };
    Some(inches * 25.4)
}

/// The dimensions a size name gives, in millimetres (a "(…)" restating it is dropped).
fn dims(size: &str) -> Option<Vec<f64>> {
    let mut s = size.to_string();
    while let (Some(a), Some(b)) = (s.find('('), s.find(')')) {
        if b < a {
            break;
        }
        s.replace_range(a..=b, "");
    }
    s.split(['x', 'X']).map(|p| swg_mm(p).or_else(|| length_mm(p))).collect()
}

/// A rectangular tube with rounded corners: outer radius twice the wall, inner radius the wall (EN 10219-2's usual
/// cold-formed corners).
fn rounded_tube(w: f64, h: f64, t: f64) -> Result<FrameProfile, String> {
    let mut p = fp::rect_tube(w, h, t)?;
    let m = |v: f64| v * 1e-3;
    let rounded_rect = |hw: f64, hh: f64, r: f64| -> Vec<k::Segment2d> {
        let (hw, hh, r) = (m(hw), m(hh), m(r).min(m(hw)).min(m(hh)));
        let c = [DVec2::new(hw - r, -hh + r), DVec2::new(hw - r, hh - r), DVec2::new(-hw + r, hh - r), DVec2::new(-hw + r, -hh + r)];
        let q = std::f64::consts::FRAC_PI_2;
        let mut out = Vec::new();
        for (i, center) in c.iter().enumerate() {
            let a0 = -q + q * i as f64;
            out.push(k::Segment2d::Arc { center: *center, radius: r, start_angle: a0, end_angle: a0 + q });
            let next = c[(i + 1) % 4];
            let from = *center + DVec2::from_angle(a0 + q) * r;
            let to = next + DVec2::from_angle(a0 + q) * r;
            out.push(k::Segment2d::Line { a: from, b: to });
        }
        out
    };
    p.outer = rounded_rect(w / 2.0, h / 2.0, 2.0 * t);
    p.holes = vec![rounded_rect(w / 2.0 - t, h / 2.0 - t, t)];
    Ok(p)
}

/// The profile of a library pick.
pub fn profile(standard: &str, profile: &str, size: &str, rounded: bool) -> Result<FrameProfile, String> {
    let bad = || format!("Unknown frame profile size \"{size}\"");
    let d = dims(size).ok_or_else(bad)?;
    let mut p = match (profile, d.as_slice()) {
        ("Rectangular bar", [w, t]) => fp::flat_bar(*w, *t),
        ("Rectangular tube", [w, h, t]) if rounded => rounded_tube(*w, *h, *t),
        ("Rectangular tube", [w, h, t]) => fp::rect_tube(*w, *h, *t),
        ("Round bar", [d, ..]) => fp::round_bar(*d),
        ("Round tube", [od, t]) => fp::round_tube(*od, *t),
        _ => return Err(bad()),
    }?;
    p.standard = standard.to_string();
    p.description = format!("{profile} {size}");
    Ok(p)
}

/// The profile a `profileSketch` library value names.
pub fn profile_of(value: &str) -> Option<Result<FrameProfile, String>> {
    let (standard, name, size, rounded) = parse_value(value)?;
    Some(profile(&standard, &name, &size, rounded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_size_builds_its_profile() {
        for s in standards() {
            for p in profiles(s) {
                let (list, default) = sizes(s, p);
                assert!(list.contains(&default), "{s} {p}: default {default}");
                for size in list {
                    let r = profile(s, p, size, false);
                    assert!(r.is_ok(), "{s} › {p} › {size}: {r:?}", r = r.err());
                }
            }
        }
    }

    #[test]
    fn sizes_read_as_published() {
        let near = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(near(&dims("20mm x 20mm x 2mm").unwrap(), &[20.0, 20.0, 2.0]));
        assert!(near(&dims("0.375in X 0.25in").unwrap(), &[9.525, 6.35]));
        assert!(near(&dims("5 1/2in (5.5)").unwrap(), &[139.7]));
        assert!(near(&dims("0.375in OD X 16SWG").unwrap(), &[9.525, 1.6256]));
        assert!(near(&dims("0.5\" x 1\" x 0.035\"").unwrap(), &[12.7, 25.4, 0.889]));
        assert!(near(&dims("16mm DIA").unwrap(), &[16.0]));
    }

    #[test]
    fn a_pick_round_trips_through_its_value() {
        let v = value("ISO", "Rectangular tube", "30mm x 30mm x 2mm", true);
        assert_eq!(parse_value(&v), Some(("ISO".into(), "Rectangular tube".into(), "30mm x 30mm x 2mm".into(), true)));
        let p = profile_of(&v).unwrap().unwrap();
        assert!((p.max.x - 0.015).abs() < 1e-12 && p.holes.len() == 1);
        // Its rounded section is a face: 30² − 26² less the corners' (4 − π)(r² − r_i²) with r = 4, r_i = 2.
        let area = p.face().unwrap().area().unwrap() * 1e6;
        let expected = 900.0 - 676.0 - (4.0 - std::f64::consts::PI) * (16.0 - 4.0);
        assert!((area - expected).abs() < 1e-6, "{area} vs {expected}");
        // No size given: the profile's default.
        assert_eq!(parse_value("frame-library:ANSI/Round bar").map(|v| v.2), Some("1/32in (0.03125)".into()));
    }
}
