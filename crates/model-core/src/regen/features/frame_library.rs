//! Frame › Sketch profile › Libraries (live behaviours "psui (2026-11-01) Frame Sketch profile picker", D134): the
//! profile library's levels (Library › Standard or Type › Profile › Size) with live's lists and defaults, and each
//! profile's geometry built from the published dimensions its size names (no Onshape sketch is used). A Frame's
//! `profileSketch` names one as `frame-library:<standard>/<profile>`, its configuration after a tab
//! (`Size=…;Rounded=true`), as the select panel stores a configured pick.

use super::frames_profiles::{self as fp, FrameProfile};
use glam::DVec2;
use noshape_kernel as k;
use std::collections::{BTreeMap, HashMap};
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

/// Live's size lists by standard and profile (dom/psui-d134-frame-profile-sizes-*.json): ANSI and ISO.
static LISTS: LazyLock<BTreeMap<String, BTreeMap<String, SizeList>>> = LazyLock::new(|| serde_json::from_str(include_str!("frame_library.json")).unwrap_or_default());

/// AISC's shapes, smallest first as live lists them (C3x3.5 first): each size's name and its dimensions in inches
/// from the AISC Shapes Database v15.0 (W / M / S / HP / C / MC / tees: d, bf, tw, tf; L: d, b, t; Pipe and round
/// HSS: OD, t design; rectangular and square HSS: Ht, B, t design).
type SizeTable = Vec<(String, Vec<f64>)>;
static AISC: LazyLock<BTreeMap<String, SizeTable>> = LazyLock::new(|| {
    let raw: BTreeMap<String, Vec<Vec<serde_json::Value>>> = serde_json::from_str(include_str!("frame_library_aisc.json")).unwrap_or_default();
    raw.into_iter()
        .map(|(p, rows)| (p, rows.into_iter().filter_map(|r| Some((r.first()?.as_str()?.to_string(), r[1..].iter().filter_map(|v| v.as_f64()).collect()))).collect()))
        .collect()
});

/// AISC's one Type and its profiles in live's order (D134).
const AISC_TYPE: &str = "AISC profiles";
const AISC_PROFILES: [&str; 14] = ["C-shape", "HP-shape", "L-angle", "M-shape", "MC-shape", "MT-shape", "Pipe", "Rectangular", "Round", "S-shape", "Square", "ST-shape", "W-shape", "WT-shape"];

/// One profile of the standards built from `frame_library_more.json` (8020, AS, Construction materials): how its
/// section is drawn (`kind`), its default size, and each size's name with its dimensions in millimetres (8020's
/// profiles have no Size and are live's sections, [`EXACT_8020`]).
struct MoreProfile {
    name: String,
    kind: String,
    default: String,
    sizes: Vec<(String, Vec<f64>)>,
}

/// 8020, AS and Construction materials: standard › Type › profiles, in live's order (dom/psui-2220-frame-*.json).
type MoreTypes = Vec<(String, Vec<MoreProfile>)>;
/// The file's rows: standard › [Type › [profile, kind, default size, [size, dimensions…]]].
type RawProfile = (String, String, String, Vec<Vec<serde_json::Value>>);
type RawStandard = (String, Vec<(String, Vec<RawProfile>)>);
static MORE: LazyLock<Vec<(String, MoreTypes)>> = LazyLock::new(|| {
    let raw: Vec<RawStandard> = serde_json::from_str(include_str!("frame_library_more.json")).unwrap_or_default();
    let row = |r: Vec<serde_json::Value>| Some((r.first()?.as_str()?.to_string(), r[1..].iter().filter_map(|v| v.as_f64()).collect()));
    raw.into_iter()
        .map(|(standard, types)| {
            let types = types.into_iter().map(|(t, ps)| (t, ps.into_iter().map(|(name, kind, default, rows)| MoreProfile { name, kind, default, sizes: rows.into_iter().filter_map(row).collect() }).collect())).collect();
            (standard, types)
        })
        .collect()
});

/// 8020's sections as live's Frames build them (live2 skasm-q1956: each end face's loops in the profile's own frame, mm):
/// profile name -> loops, the outer first; a segment is ["L", a, b], ["A", a, mid, b] or ["C", centre, r] flattened.
static EXACT_8020: LazyLock<HashMap<String, Vec<Vec<Vec<serde_json::Value>>>>> = LazyLock::new(|| {
    #[derive(serde::Deserialize)]
    struct Section {
        loops: Vec<Vec<Vec<serde_json::Value>>>,
    }
    let raw: HashMap<String, HashMap<String, Section>> = serde_json::from_str(include_str!("frame_library_8020.json")).unwrap_or_default();
    raw.into_values().flatten().map(|(name, s)| (name, s.loops)).collect()
});

/// An 8020 profile's exact section ([`EXACT_8020`]).
fn exact_8020(profile: &str) -> Option<Result<FrameProfile, String>> {
    let loops = EXACT_8020.get(profile)?;
    let m = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0) * 1e-3;
    let p = |s: &[serde_json::Value], i: usize| DVec2::new(m(&s[i]), m(&s[i + 1]));
    let mut built: Vec<Vec<k::Segment2d>> = Vec::new();
    for l in loops {
        let mut segs = Vec::new();
        for s in l {
            let seg = match (s.first().and_then(|k| k.as_str()), s.len()) {
                (Some("L"), 5) => k::Segment2d::Line { a: p(s, 1), b: p(s, 3) },
                (Some("A"), 7) => k::Segment2d::Arc3 { a: p(s, 1), mid: p(s, 3), b: p(s, 5) },
                (Some("C"), 4) => k::Segment2d::Circle { center: p(s, 1), radius: m(&s[3]) },
                _ => return Some(Err(format!("Bad section data for 8020 {profile}"))),
            };
            segs.push(seg);
        }
        built.push(segs);
    }
    let mut it = built.into_iter();
    let outer = it.next()?;
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for s in &outer {
        let pts = match *s {
            k::Segment2d::Line { a, b } => vec![a, b],
            k::Segment2d::Arc3 { a, mid, b } => arc3_extremes(a, mid, b),
            k::Segment2d::Circle { center, radius } => vec![center - DVec2::splat(radius), center + DVec2::splat(radius)],
            _ => Vec::new(),
        };
        for q in pts {
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    Some(Ok(FrameProfile { standard: String::new(), description: String::new(), outer, holes: it.collect(), min: lo, max: hi }))
}

/// The circle through an arc's ends `a`, `b` and a point `mid` on it: its centre, radius, and the arc from `a` as a
/// signed sweep (positive counter-clockwise); None when the points are in line.
fn arc3(a: DVec2, mid: DVec2, b: DVec2) -> Option<(DVec2, f64, f64)> {
    let d = 2.0 * (a.x * (mid.y - b.y) + mid.x * (b.y - a.y) + b.x * (a.y - mid.y));
    if d.abs() < 1e-30 {
        return None;
    }
    let sq = |p: DVec2| p.length_squared();
    let c = DVec2::new((sq(a) * (mid.y - b.y) + sq(mid) * (b.y - a.y) + sq(b) * (a.y - mid.y)) / d, (sq(a) * (b.x - mid.x) + sq(mid) * (a.x - b.x) + sq(b) * (mid.x - a.x)) / d);
    let tau = std::f64::consts::TAU;
    let ang = |p: DVec2| (p.y - c.y).atan2(p.x - c.x);
    let (t0, tm, t1) = (ang(a), ang(mid), ang(b));
    let ccw = (t1 - t0).rem_euclid(tau);
    let sweep = if (tm - t0).rem_euclid(tau) < ccw { ccw } else { ccw - tau };
    Some((c, a.distance(c), sweep))
}

/// The ends of an arc through three points and the points where it reaches furthest along x and y.
fn arc3_extremes(a: DVec2, mid: DVec2, b: DVec2) -> Vec<DVec2> {
    let Some((c, r, sweep)) = arc3(a, mid, b) else { return vec![a, b] };
    let tau = std::f64::consts::TAU;
    let t0 = (a.y - c.y).atan2(a.x - c.x);
    let (start, sweep) = if sweep >= 0.0 { (t0, sweep) } else { (t0 + sweep, -sweep) };
    let mut out = vec![a, b];
    for q in 0..4 {
        let t = q as f64 * std::f64::consts::FRAC_PI_2;
        if (t - start).rem_euclid(tau) <= sweep {
            out.push(c + DVec2::new(t.cos(), t.sin()) * r);
        }
    }
    out
}

fn more_profile(standard: &str, profile: &str) -> Option<&'static MoreProfile> {
    MORE.iter().find(|(s, _)| s == standard)?.1.iter().flat_map(|(_, ps)| ps).find(|p| p.name == profile)
}

/// The library's standards, in the picker's order (live: 8020, AISC, ANSI, AS, Construction materials, ISO).
pub fn standards() -> Vec<&'static str> {
    let mut all: Vec<&'static str> = std::iter::once("AISC").chain(LISTS.keys().map(|s| s.as_str())).chain(MORE.iter().map(|(s, _)| s.as_str())).collect();
    all.sort_by_key(|s| s.to_lowercase());
    all
}

/// A standard's Type level (live: AISC › "AISC profiles", AS's two, 8020's series); none for ANSI and ISO, whose
/// second level is itself labelled "Type".
pub fn types(standard: &str) -> Vec<&'static str> {
    if standard == "AISC" {
        return vec![AISC_TYPE];
    }
    MORE.iter().find(|(s, _)| s == standard).map(|(_, ts)| ts.iter().map(|(t, _)| t.as_str()).collect()).unwrap_or_default()
}

/// The second level's label for `standard`: "Standard" above a Type level, else "Type" (live D134).
pub fn standard_label(standard: &str) -> &'static str {
    if types(standard).is_empty() { "Type" } else { "Standard" }
}

/// A standard's profiles of every Type, in live's order.
pub fn profiles(standard: &str) -> Vec<&'static str> {
    if standard == "AISC" {
        return AISC_PROFILES.iter().copied().filter(|p| AISC.contains_key(*p)).collect();
    }
    if let Some((_, ts)) = MORE.iter().find(|(s, _)| s == standard) {
        return ts.iter().flat_map(|(_, ps)| ps.iter().map(|p| p.name.as_str())).collect();
    }
    const ORDER: [&str; 4] = ["Rectangular bar", "Rectangular tube", "Round bar", "Round tube"];
    let Some(s) = LISTS.get(standard) else { return Vec::new() };
    ORDER.iter().copied().filter(|p| s.contains_key(*p)).collect()
}

/// The profiles of one Type of a standard (a standard without a Type level: all of them).
pub fn type_profiles(standard: &str, ty: &str) -> Vec<&'static str> {
    match MORE.iter().find(|(s, _)| s == standard) {
        Some((_, ts)) => ts.iter().find(|(t, _)| t == ty).map(|(_, ps)| ps.iter().map(|p| p.name.as_str()).collect()).unwrap_or_default(),
        None => profiles(standard),
    }
}

/// The Type a profile is under (profile names are unique within a standard).
pub fn type_of(standard: &str, profile: &str) -> Option<&'static str> {
    types(standard).into_iter().find(|t| type_profiles(standard, t).contains(&profile))
}

/// A profile's sizes and the default one (AISC's: the first); none for a profile that is its own size (8020's).
pub fn sizes(standard: &str, profile: &str) -> (Vec<&'static str>, &'static str) {
    if standard == "AISC" {
        let list: Vec<&'static str> = AISC.get(profile).map(|l| l.iter().map(|(n, _)| n.as_str()).collect()).unwrap_or_default();
        let first = list.first().copied().unwrap_or("");
        return (list, first);
    }
    if let Some(p) = more_profile(standard, profile) {
        if p.default.is_empty() {
            return (Vec::new(), "");
        }
        return (p.sizes.iter().map(|(n, _)| n.as_str()).collect(), p.default.as_str());
    }
    match LISTS.get(standard).and_then(|s| s.get(profile)) {
        Some(l) => (l.sizes.iter().map(|s| s.as_str()).collect(), l.default.as_str()),
        None => (Vec::new(), ""),
    }
}

/// A tee: the flange `bf` × `tf` on top, the stem `tw` wide down to the bottom, `d` deep (mm).
fn tee(d: f64, bf: f64, tw: f64, tf: f64) -> Result<FrameProfile, String> {
    if tw >= bf || tf >= d {
        return Err("Tee stem or flange is too thick".into());
    }
    let m = |v: f64| v * 1e-3;
    let pts = [(-tw / 2.0, 0.0), (tw / 2.0, 0.0), (tw / 2.0, d - tf), (bf / 2.0, d - tf), (bf / 2.0, d), (-bf / 2.0, d), (-bf / 2.0, d - tf), (-tw / 2.0, d - tf)];
    let p: Vec<DVec2> = pts.iter().map(|&(x, y)| DVec2::new(m(x), m(y))).collect();
    let outer = (0..p.len()).map(|i| k::Segment2d::Line { a: p[i], b: p[(i + 1) % p.len()] }).collect();
    Ok(FrameProfile { standard: String::new(), description: String::new(), outer, holes: Vec::new(), min: DVec2::new(m(-bf / 2.0), 0.0), max: DVec2::new(m(bf / 2.0), m(d)) })
}

/// An AISC shape from its published dimensions (inches).
fn aisc_profile(profile: &str, d: &[f64]) -> Result<FrameProfile, String> {
    let mm: Vec<f64> = d.iter().map(|v| v * 25.4).collect();
    match (profile, mm.as_slice()) {
        ("W-shape" | "M-shape" | "S-shape" | "HP-shape", [d, bf, tw, tf]) => fp::ibeam(*d, *bf, *tw, *tf),
        ("C-shape" | "MC-shape", [d, bf, tw, tf]) => fp::channel(*d, *bf, *tw, *tf),
        ("WT-shape" | "MT-shape" | "ST-shape", [d, bf, tw, tf]) => tee(*d, *bf, *tw, *tf),
        ("L-angle", [a, b, t]) => fp::angle(*a, *b, *t),
        ("Pipe" | "Round", [od, t]) => fp::round_tube(*od, *t),
        // HSS: AISC's outside corner radius is twice the design wall.
        ("Rectangular" | "Square", [ht, b, t]) => rounded_tube(*b, *ht, *t),
        _ => Err(format!("No dimensions for AISC {profile}")),
    }
}

/// Whether a profile has the Rounded checkbox (live: ISO's Rectangular tube).
pub fn has_rounded(standard: &str, profile: &str) -> bool {
    standard == "ISO" && profile == "Rectangular tube"
}

/// The `profileSketch` value of a library profile with its configuration.
pub fn value(standard: &str, profile: &str, size: &str, rounded: bool) -> String {
    let mut v = format!("{PREFIX}{standard}/{profile}");
    if !size.is_empty() {
        v.push_str(&format!("\t{SIZE}={size}"));
    }
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

/// A rectangle `2·hw` × `2·hh` (mm) about the origin with corners of radius `r`: four arcs and four lines (metres).
fn rounded_rect(hw: f64, hh: f64, r: f64) -> Vec<k::Segment2d> {
    let m = |v: f64| v * 1e-3;
    let (hw, hh, r) = (m(hw), m(hh), m(r).min(m(hw)).min(m(hh)));
    let c = [DVec2::new(hw - r, -hh + r), DVec2::new(hw - r, hh - r), DVec2::new(-hw + r, hh - r), DVec2::new(-hw + r, -hh + r)];
    let q = std::f64::consts::FRAC_PI_2;
    let mut out = Vec::new();
    for (i, center) in c.iter().enumerate() {
        let a0 = -q + q * i as f64;
        if r > 0.0 {
            out.push(k::Segment2d::Arc { center: *center, radius: r, start_angle: a0, end_angle: a0 + q });
        }
        let (a, b) = (*center + DVec2::from_angle(a0 + q) * r, c[(i + 1) % 4] + DVec2::from_angle(a0 + q) * r);
        // A side all corner (a stadium's ends) has no line.
        if a.distance(b) > 1e-12 {
            out.push(k::Segment2d::Line { a, b });
        }
    }
    out
}

/// A rectangular tube `w` × `h` with wall `t` and corners of outer radius `ro`, inner `ri` (mm).
fn tube_with_corners(w: f64, h: f64, t: f64, ro: f64, ri: f64) -> Result<FrameProfile, String> {
    let mut p = fp::rect_tube(w, h, t)?;
    p.outer = rounded_rect(w / 2.0, h / 2.0, ro);
    p.holes = vec![rounded_rect(w / 2.0 - t, h / 2.0 - t, ri)];
    Ok(p)
}

/// A rectangular tube with rounded corners: outer radius twice the wall, inner radius the wall (EN 10219-2's usual
/// cold-formed corners).
fn rounded_tube(w: f64, h: f64, t: f64) -> Result<FrameProfile, String> {
    tube_with_corners(w, h, t, 2.0 * t, t)
}

/// A profile of 8020, AS or Construction materials from its kind and its row's dimensions (mm).
fn more_section(kind: &str, d: &[f64]) -> Result<FrameProfile, String> {
    match (kind, d) {
        ("bar", [w, h]) => fp::flat_bar(*w, *h),
        ("chs", [od, t]) => fp::round_tube(*od, *t),
        // AS/NZS 1163 cold-formed hollow sections: outside corner radius 2t up to 3 mm walls, 2.5t above.
        ("rhs", [h, w, t]) => {
            let ro = if *t <= 3.0 { 2.0 * t } else { 2.5 * t };
            tube_with_corners(*w, *h, *t, ro, ro - t)
        }
        // AS's rail hollow section: a flat-sided oval, its ends half circles (DuraGal Rail).
        ("stadium", [w, h, t]) => tube_with_corners(*w, *h, *t, h / 2.0, h / 2.0 - t),
        ("angle", [a, b, t]) => fp::angle(*a, *b, *t),
        ("channel", [h, b, t]) => fp::channel(*h, *b, *t, *t),
        ("channel", [h, b, tw, tf]) => fp::channel(*h, *b, *tw, *tf),
        ("ibeam", [h, b, tw, tf]) => fp::ibeam(*h, *b, *tw, *tf),
        _ => Err(format!("No dimensions for a {kind} profile")),
    }
}

/// The profile of a library pick.
pub fn profile(standard: &str, profile: &str, size: &str, rounded: bool) -> Result<FrameProfile, String> {
    let bad = || format!("Unknown frame profile size \"{size}\"");
    if standard == "AISC" {
        let d = AISC.get(profile).and_then(|l| l.iter().find(|(n, _)| n == size)).map(|(_, d)| d.clone()).ok_or_else(bad)?;
        let mut p = aisc_profile(profile, &d)?;
        p.standard = standard.to_string();
        p.description = format!("{profile} {size}");
        return Ok(p);
    }
    if let Some(mp) = more_profile(standard, profile) {
        if standard == "8020"
            && let Some(p) = exact_8020(profile)
        {
            let mut p = p?;
            p.standard = standard.to_string();
            p.description = profile.to_string();
            return Ok(p);
        }
        let d = match mp.sizes.as_slice() {
            [(_, d)] if mp.default.is_empty() => d,
            rows => &rows.iter().find(|(n, _)| n == size).ok_or_else(bad)?.1,
        };
        let mut p = more_section(&mp.kind, d)?;
        p.standard = standard.to_string();
        p.description = if size.is_empty() { profile.to_string() } else { format!("{profile} {size}") };
        return Ok(p);
    }
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

/// The library's preview of a profile (live: the profile's sketch seen from above, a pale region with its outline and
/// the origin): an SVG `width` × `height` px, the profile fitted with a margin.
pub fn preview_svg(p: &FrameProfile, width: f64, height: f64) -> String {
    let (lo, hi) = (p.min.min(DVec2::ZERO), p.max.max(DVec2::ZERO));
    let size = (hi - lo).max(DVec2::splat(1e-9));
    let scale = ((width * 0.8) / size.x).min((height * 0.8) / size.y);
    let centre = (lo + hi) / 2.0;
    let at = |q: DVec2| DVec2::new(width / 2.0 + (q.x - centre.x) * scale, height / 2.0 - (q.y - centre.y) * scale);
    let mut d = String::new();
    let mut push = |s: String| {
        d.push_str(&s);
        d.push(' ');
    };
    for lp in std::iter::once(&p.outer).chain(p.holes.iter()) {
        let mut first = true;
        for seg in lp {
            match seg {
                k::Segment2d::Line { a, b } => {
                    let (a, b) = (at(*a), at(*b));
                    if first {
                        push(format!("M{:.2},{:.2}", a.x, a.y));
                    }
                    push(format!("L{:.2},{:.2}", b.x, b.y));
                }
                k::Segment2d::Arc { center, radius, start_angle, end_angle } => {
                    let end = if end_angle <= start_angle { end_angle + std::f64::consts::TAU } else { *end_angle };
                    let (a, b) = (at(*center + DVec2::from_angle(*start_angle) * *radius), at(*center + DVec2::from_angle(end) * *radius));
                    if first {
                        push(format!("M{:.2},{:.2}", a.x, a.y));
                    }
                    // Counter-clockwise in the profile is clockwise on the y-down page: sweep 0.
                    let r = radius * scale;
                    push(format!("A{r:.2},{r:.2} 0 {} 0 {:.2},{:.2}", u8::from(end - start_angle > std::f64::consts::PI), b.x, b.y));
                }
                k::Segment2d::Arc3 { a, mid, b } => {
                    let (pa, pb) = (at(*a), at(*b));
                    if first {
                        push(format!("M{:.2},{:.2}", pa.x, pa.y));
                    }
                    match arc3(*a, *mid, *b) {
                        Some((_, r, sweep)) => {
                            let r = r * scale;
                            push(format!("A{r:.2},{r:.2} 0 {} {} {:.2},{:.2}", u8::from(sweep.abs() > std::f64::consts::PI), u8::from(sweep < 0.0), pb.x, pb.y));
                        }
                        None => push(format!("L{:.2},{:.2}", pb.x, pb.y)),
                    }
                }
                k::Segment2d::Circle { center, radius } => {
                    let (c, r) = (at(*center), radius * scale);
                    push(format!("M{:.2},{:.2} A{r:.2},{r:.2} 0 1 0 {:.2},{:.2} A{r:.2},{r:.2} 0 1 0 {:.2},{:.2}", c.x + r, c.y, c.x - r, c.y, c.x + r, c.y));
                }
                _ => {}
            }
            first = false;
        }
        push("Z".into());
    }
    let o = at(DVec2::ZERO);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><path d=\"{}\" fill=\"#e3e9ea\" fill-rule=\"evenodd\" stroke=\"#55626a\" stroke-width=\"1.2\"/><circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"2\" fill=\"none\" stroke=\"#55626a\" stroke-width=\"1\"/></svg>",
        d.trim_end(),
        o.x,
        o.y
    )
}

/// The library's profiles whose names hold `query` (case-insensitive), as live's search lists them: the earlier the
/// match the higher ("tube": Round tube before Rectangular tube), then by standard.
pub fn search(query: &str) -> Vec<(&'static str, &'static str)> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let q = q.as_str();
    let mut hits: Vec<(usize, &'static str, &'static str)> = standards().into_iter().flat_map(|s| profiles(s).into_iter().filter_map(move |p| p.to_lowercase().find(q).map(|i| (i, s, p)))).collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(b.2)).then(a.1.cmp(b.1)));
    hits.into_iter().map(|(_, s, p)| (s, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_size_builds_its_profile() {
        for s in standards() {
            for p in profiles(s) {
                let (list, default) = sizes(s, p);
                assert!(list.contains(&default) || list.is_empty() && default.is_empty(), "{s} {p}: default {default}");
                let list = if list.is_empty() { vec![""] } else { list };
                for size in list {
                    let r = profile(s, p, size, false);
                    assert!(r.is_ok(), "{s} › {p} › {size}: {r:?}", r = r.err());
                    // The sections drawn here (tees, rounded tubes, T-slots, a stadium) are faces the kernel builds.
                    if !matches!(s, "ANSI" | "ISO") {
                        assert!(r.unwrap().face().is_ok_and(|f| f.area().is_ok_and(|a| a > 0.0)), "{s} › {p} › {size}: no face");
                    }
                }
            }
        }
    }

    /// Live's lists (2220): 8020's seven series with 1001 first (live's fresh default), AS's two Types, Construction
    /// materials' woods with 2" x 4" by default.
    #[test]
    fn live_2220_lists() {
        assert_eq!(types("8020"), ["10 Series", "15 Series", "20 Series", "25 Series", "30 Series", "40 Series", "45 Series"]);
        assert_eq!((type_profiles("8020", "10 Series").len(), type_profiles("8020", "10 Series")[0], sizes("8020", "1001")), (22, "1001", (vec![], "")));
        assert_eq!(profiles("8020").len(), 108);
        assert_eq!(type_of("8020", "40-4080-Lite"), Some("40 Series"));
        assert_eq!(types("AS"), ["Cold formed hollow sections and profiles", "Hot rolled and structural steel"]);
        assert_eq!(type_profiles("AS", "Hot rolled and structural steel").len(), 9);
        assert_eq!(sizes("AS", "Universal beam").1, "150 UB 14");
        assert_eq!((types("Construction materials"), sizes("Construction materials", "Lumber Inch").1), (vec!["Construction wood"], "2\" x 4\""));
        assert_eq!(value("8020", "1001", "", false), "frame-library:8020/1001");
        assert_eq!(parse_value("frame-library:8020/1001"), Some(("8020".into(), "1001".into(), String::new(), false)));
        let area = |s: &str, p: &str, z: &str| profile(s, p, z, false).unwrap().face().unwrap().area().unwrap() * 1e6;
        // A 2x4 is dressed to 1.5" x 3.5".
        assert!((area("Construction materials", "Lumber Inch", "2\" x 4\"") - 1.5 * 3.5 * 645.16).abs() < 1e-6);
        // 150 UB 14: d 150, bf 75, tf 7, tw 5 (no root radii).
        assert!((area("AS", "Universal beam", "150 UB 14") - (2.0 * 75.0 * 7.0 + 136.0 * 5.0)).abs() < 1e-6);
    }

    /// Every 8020 section is live's Frame end face (live2 skasm-q1956): its area to live's six digits, its preview with
    /// the arcs (1001: the four rounded corners, the slot lips and the grooves).
    #[test]
    fn sections_8020_are_lives() {
        let raw: HashMap<String, HashMap<String, serde_json::Value>> = serde_json::from_str(include_str!("frame_library_8020.json")).unwrap();
        let mut n = 0;
        for (name, s) in raw.into_values().flatten() {
            let live = s["area"].as_f64().unwrap();
            let ours = profile("8020", &name, "", false).unwrap().face().unwrap().area().unwrap() * 1e6;
            assert!((ours - live).abs() < 1e-5 * live, "{name}: {ours} vs live {live} mm²");
            n += 1;
        }
        assert_eq!(n, 108);
        let p = profile("8020", "1001", "", false).unwrap();
        assert!(p.holes.len() == 4 && ((p.max - p.min) * 1e3 - DVec2::splat(25.4)).abs().max_element() < 1e-9, "{} holes, {:?}", p.holes.len(), p.max - p.min);
        assert!(preview_svg(&p, 240.0, 136.0).matches('A').count() >= 8);
    }

    #[test]
    fn aisc_lists_and_shapes() {
        assert_eq!(standards(), ["8020", "AISC", "ANSI", "AS", "Construction materials", "ISO"]);
        assert_eq!((types("AISC"), standard_label("AISC"), standard_label("ISO")), (vec!["AISC profiles"], "Standard", "Type"));
        assert_eq!(profiles("AISC").len(), 14);
        let (c, default) = sizes("AISC", "C-shape");
        assert_eq!((c.len(), default, &c[..4]), (32, "C3x3.5", &["C3x3.5", "C3x4.1", "C3x5", "C3x6"][..]));
        // W8x31: d 8.0, bf 8.0, tw 0.285, tf 0.435 in: its I section's area.
        let w = profile("AISC", "W-shape", "W8x31", false).unwrap();
        let a = w.face().unwrap().area().unwrap() / (0.0254 * 0.0254);
        let expected = 2.0 * 8.0 * 0.435 + (8.0 - 2.0 * 0.435) * 0.285;
        assert!((a - expected).abs() < 1e-6, "{a} vs {expected}");
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
    fn search_and_preview() {
        assert_eq!(search("tube"), vec![("ANSI", "Round tube"), ("ISO", "Round tube"), ("ANSI", "Rectangular tube"), ("ISO", "Rectangular tube")]);
        // Live D134: "angle" ↦ AISC › L-angle first.
        assert_eq!(search("angle").first(), Some(&("AISC", "L-angle")));
        assert!(search(" ").is_empty());
        let p = profile("ISO", "Rectangular tube", "30mm x 30mm x 2mm", true).unwrap();
        let svg = preview_svg(&p, 240.0, 136.0);
        // The outer loop and the hole, each four arcs and four lines.
        assert_eq!(svg.matches('A').count(), 8, "{svg}");
        assert_eq!(svg.matches('Z').count(), 2, "{svg}");
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
