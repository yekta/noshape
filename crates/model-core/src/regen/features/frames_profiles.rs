//! Built-in frame profile library (03b FR-02): square / rectangular tubes
//! (SHS / RHS), round tubes (CHS), round and flat bars, equal angles (L),
//! channels (UPN) and I-beams (IPE), in common sizes. Profiles are given in
//! millimeters and built in the XY plane in meters.
//!
//! The frame feature's `profileSketch` text selects a profile either by a
//! library name ("SHS 40x40x3", "IPE 100") or generically by kind and
//! dimensions in mm ("square tube 40x3", "rect tube 60x40x3", "round tube
//! 42.4x3.2", "angle 40x40x4", "channel 100x50x6x8.5", "ibeam 100x55x4.1x5.7",
//! "flat bar 40x10", "round bar 20").

use glam::DVec2;
use noshape_kernel as k;

/// Cross-section of a frame member (meters, XY plane).
#[derive(Clone, Debug)]
pub struct FrameProfile {
    pub standard: String,
    pub description: String,
    pub outer: Vec<k::Segment2d>,
    pub holes: Vec<Vec<k::Segment2d>>,
    pub min: DVec2,
    pub max: DVec2,
}

impl FrameProfile {
    /// Planar face of the profile in the XY plane.
    pub fn face(&self) -> Result<k::Shape, k::KernelError> {
        k::make_face_2d(&k::Plane::xy(), &self.outer, &self.holes)
    }

    /// Alignment point `index` of the 9-point bounding box grid (0..8,
    /// row-major from the bottom left; 4 = center).
    pub fn alignment_point(&self, index: i64) -> DVec2 {
        let i = index.clamp(0, 8) as usize;
        let (col, row) = (i % 3, i / 3);
        let lerp = |a: f64, b: f64, t: usize| a + (b - a) * t as f64 * 0.5;
        DVec2::new(lerp(self.min.x, self.max.x, col), lerp(self.min.y, self.max.y, row))
    }

    /// Largest distance of the bounding box corners from a point.
    pub fn radius_about(&self, p: DVec2) -> f64 {
        [self.min, self.max, DVec2::new(self.min.x, self.max.y), DVec2::new(self.max.x, self.min.y)].iter().map(|c| (*c - p).length()).fold(0.0, f64::max)
    }
}

fn mm(v: f64) -> f64 {
    v * 1e-3
}

fn poly(pts: &[(f64, f64)]) -> Vec<k::Segment2d> {
    let p: Vec<DVec2> = pts.iter().map(|&(x, y)| DVec2::new(mm(x), mm(y))).collect();
    (0..p.len()).map(|i| k::Segment2d::Line { a: p[i], b: p[(i + 1) % p.len()] }).collect()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<k::Segment2d> {
    poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

fn make(standard: &str, description: String, outer: Vec<k::Segment2d>, holes: Vec<Vec<k::Segment2d>>, min: (f64, f64), max: (f64, f64)) -> FrameProfile {
    FrameProfile { standard: standard.into(), description, outer, holes, min: DVec2::new(mm(min.0), mm(min.1)), max: DVec2::new(mm(max.0), mm(max.1)) }
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn rect_tube(w: f64, h: f64, t: f64) -> Result<FrameProfile, String> {
    if t <= 0.0 || 2.0 * t >= w.min(h) {
        return Err("Tube wall thickness is too large".into());
    }
    let (hw, hh) = (w / 2.0, h / 2.0);
    let name = if (w - h).abs() < 1e-9 { format!("SHS {}x{}x{}", fmt(w), fmt(h), fmt(t)) } else { format!("RHS {}x{}x{}", fmt(w), fmt(h), fmt(t)) };
    Ok(make("EN 10219", name, rect(-hw, -hh, hw, hh), vec![rect(-hw + t, -hh + t, hw - t, hh - t)], (-hw, -hh), (hw, hh)))
}

pub fn round_tube(d: f64, t: f64) -> Result<FrameProfile, String> {
    if t <= 0.0 || 2.0 * t >= d {
        return Err("Tube wall thickness is too large".into());
    }
    let r = mm(d / 2.0);
    let outer = vec![k::Segment2d::Circle { center: DVec2::ZERO, radius: r }];
    let hole = vec![k::Segment2d::Circle { center: DVec2::ZERO, radius: r - mm(t) }];
    Ok(make("EN 10219", format!("CHS {}x{}", fmt(d), fmt(t)), outer, vec![hole], (-d / 2.0, -d / 2.0), (d / 2.0, d / 2.0)))
}

pub fn round_bar(d: f64) -> Result<FrameProfile, String> {
    let outer = vec![k::Segment2d::Circle { center: DVec2::ZERO, radius: mm(d / 2.0) }];
    Ok(make("EN 10060", format!("Round bar {}", fmt(d)), outer, vec![], (-d / 2.0, -d / 2.0), (d / 2.0, d / 2.0)))
}

pub fn flat_bar(w: f64, h: f64) -> Result<FrameProfile, String> {
    Ok(make("EN 10058", format!("Flat bar {}x{}", fmt(w), fmt(h)), rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0), vec![], (-w / 2.0, -h / 2.0), (w / 2.0, h / 2.0)))
}

pub fn angle(a: f64, b: f64, t: f64) -> Result<FrameProfile, String> {
    if t <= 0.0 || t >= a.min(b) {
        return Err("Angle thickness is too large".into());
    }
    let outer = poly(&[(0.0, 0.0), (a, 0.0), (a, t), (t, t), (t, b), (0.0, b)]);
    Ok(make("EN 10056", format!("L {}x{}x{}", fmt(a), fmt(b), fmt(t)), outer, vec![], (0.0, 0.0), (a, b)))
}

pub fn channel(h: f64, b: f64, tw: f64, tf: f64) -> Result<FrameProfile, String> {
    if tw >= b || 2.0 * tf >= h {
        return Err("Channel web or flange is too thick".into());
    }
    let outer = poly(&[(0.0, 0.0), (b, 0.0), (b, tf), (tw, tf), (tw, h - tf), (b, h - tf), (b, h), (0.0, h)]);
    Ok(make("EN 10279", format!("C {}x{}x{}x{}", fmt(h), fmt(b), fmt(tw), fmt(tf)), outer, vec![], (0.0, 0.0), (b, h)))
}

pub fn ibeam(h: f64, b: f64, tw: f64, tf: f64) -> Result<FrameProfile, String> {
    if tw >= b || 2.0 * tf >= h {
        return Err("I-beam web or flange is too thick".into());
    }
    let (hb, hh, hw) = (b / 2.0, h / 2.0, tw / 2.0);
    let outer = poly(&[
        (-hb, -hh),
        (hb, -hh),
        (hb, -hh + tf),
        (hw, -hh + tf),
        (hw, hh - tf),
        (hb, hh - tf),
        (hb, hh),
        (-hb, hh),
        (-hb, hh - tf),
        (-hw, hh - tf),
        (-hw, -hh + tf),
        (-hb, -hh + tf),
    ]);
    Ok(make("EN 10365", format!("I {}x{}x{}x{}", fmt(h), fmt(b), fmt(tw), fmt(tf)), outer, vec![], (-hb, -hh), (hb, hh)))
}

/// Named library entries (name, builder input).
pub fn library() -> Vec<(String, FrameProfile)> {
    let mut out = Vec::new();
    let mut add = |name: String, p: Result<FrameProfile, String>| {
        if let Ok(mut p) = p {
            p.description = name.clone();
            out.push((name, p));
        }
    };
    for (a, t) in [(20.0, 2.0), (25.0, 2.5), (30.0, 3.0), (40.0, 3.0), (40.0, 4.0), (50.0, 4.0), (60.0, 4.0), (80.0, 5.0), (100.0, 6.0)] {
        add(format!("SHS {}x{}x{}", fmt(a), fmt(a), fmt(t)), rect_tube(a, a, t));
    }
    for (w, h, t) in [(40.0, 20.0, 2.0), (50.0, 30.0, 3.0), (60.0, 40.0, 3.0), (80.0, 40.0, 4.0), (100.0, 50.0, 5.0)] {
        add(format!("RHS {}x{}x{}", fmt(w), fmt(h), fmt(t)), rect_tube(w, h, t));
    }
    for (d, t) in [(21.3, 2.3), (26.9, 2.6), (33.7, 3.2), (42.4, 3.2), (48.3, 3.6), (60.3, 4.0)] {
        add(format!("CHS {}x{}", fmt(d), fmt(t)), round_tube(d, t));
    }
    for d in [10.0, 16.0, 20.0, 25.0] {
        add(format!("Round bar {}", fmt(d)), round_bar(d));
    }
    for (w, h) in [(30.0, 5.0), (40.0, 10.0), (50.0, 10.0)] {
        add(format!("Flat bar {}x{}", fmt(w), fmt(h)), flat_bar(w, h));
    }
    for (a, t) in [(20.0, 3.0), (30.0, 3.0), (40.0, 4.0), (50.0, 5.0), (60.0, 6.0)] {
        add(format!("L {}x{}x{}", fmt(a), fmt(a), fmt(t)), angle(a, a, t));
    }
    for (n, h, b, tw, tf) in [(50, 50.0, 38.0, 5.0, 7.0), (65, 65.0, 42.0, 5.5, 7.5), (80, 80.0, 45.0, 6.0, 8.0), (100, 100.0, 50.0, 6.0, 8.5), (120, 120.0, 55.0, 7.0, 9.0)] {
        add(format!("UPN {n}"), channel(h, b, tw, tf));
    }
    for (n, h, b, tw, tf) in [(80, 80.0, 46.0, 3.8, 5.2), (100, 100.0, 55.0, 4.1, 5.7), (120, 120.0, 64.0, 4.4, 6.3), (140, 140.0, 73.0, 4.7, 6.9), (160, 160.0, 82.0, 5.0, 7.4)] {
        add(format!("IPE {n}"), ibeam(h, b, tw, tf));
    }
    out
}

/// Profile from a library name or a generic "kind dims" description.
pub fn parse(text: &str) -> Result<FrameProfile, String> {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let want = norm(text);
    let want = want.strip_prefix("iso ").unwrap_or(&want).to_string();
    if let Some((_, p)) = library().into_iter().find(|(n, _)| norm(n) == want) {
        return Ok(p);
    }
    // Generic: kind followed by dimensions separated by 'x'.
    let split = want.find(|c: char| c.is_ascii_digit()).ok_or_else(|| format!("Unknown frame profile \"{text}\""))?;
    let (kind, dims) = want.split_at(split);
    let kind = kind.trim().trim_end_matches(['-', ':']).trim();
    let dims: Vec<f64> = dims.trim().trim_end_matches("mm").split(['x', '*', '×']).map(|s| s.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("Bad dimensions in frame profile \"{text}\""))?;
    let d = |i: usize| dims.get(i).copied();
    let bad = || format!("Wrong number of dimensions for frame profile \"{text}\"");
    let p = match kind {
        "shs" | "square tube" | "square" => match dims.len() {
            2 => rect_tube(dims[0], dims[0], dims[1]),
            3 => rect_tube(dims[0], dims[1], dims[2]),
            _ => return Err(bad()),
        },
        "rhs" | "rect tube" | "rectangular tube" | "rectangle tube" => match dims.len() {
            3 => rect_tube(dims[0], dims[1], dims[2]),
            _ => return Err(bad()),
        },
        "chs" | "round tube" | "pipe" | "tube" => round_tube(d(0).ok_or_else(bad)?, d(1).ok_or_else(bad)?),
        "round bar" | "rod" | "bar round" => round_bar(d(0).ok_or_else(bad)?),
        "flat bar" | "bar" | "rect bar" | "rectangular bar" | "flat" => flat_bar(d(0).ok_or_else(bad)?, d(1).ok_or_else(bad)?),
        "l" | "angle" => match dims.len() {
            2 => angle(dims[0], dims[0], dims[1]),
            3 => angle(dims[0], dims[1], dims[2]),
            _ => return Err(bad()),
        },
        "c" | "channel" | "upn" => match dims.len() {
            4 => channel(dims[0], dims[1], dims[2], dims[3]),
            _ => return Err(bad()),
        },
        "i" | "ibeam" | "i-beam" | "i beam" | "ipe" => match dims.len() {
            4 => ibeam(dims[0], dims[1], dims[2], dims[3]),
            _ => return Err(bad()),
        },
        _ => return Err(format!("Unknown frame profile \"{text}\"")),
    }?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_library_and_generic() {
        let p = parse("SHS 40x40x3").unwrap();
        assert!((p.max.x - 0.02).abs() < 1e-12);
        assert_eq!(p.holes.len(), 1);
        let q = parse("square tube 40x3").unwrap();
        assert_eq!(q.description, "SHS 40x40x3");
        assert!(parse("IPE 100").is_ok());
        assert!(parse("angle 40x40x4").is_ok());
        assert!(parse("channel 100x50x6x8.5").is_ok());
        assert!(parse("round tube 42.4x3.2").is_ok());
        assert!(parse("nonsense").is_err());
        assert!(library().len() > 30);
        let c = p.alignment_point(4);
        assert!(c.length() < 1e-12);
        assert!((p.alignment_point(0) - p.min).length() < 1e-12);
    }
}
