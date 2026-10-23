//! Hole and thread standard tables (compact port of the main entries of
//! FeatureScript's `holetables.gen.fs`).
//!
//! A table is a cascade of levels; a *path* is the list of option ids chosen
//! so far (one per level) and is what a [`ParamValue::Lookup`] stores. The
//! leaf of a complete path yields a [`HoleSpec`].
//!
//! Covered: ANSI (#0 .. 1", UNC + UNF) and ISO (M1.6 .. M24, coarse + fine)
//! for Drilled / Clearance / Tapped (straight tap) holes, thread classes and
//! external threads. Not covered yet: pipe taps (NPT, G/RP, R/BSPT), PEM
//! fasteners, sizes outside the ranges above, UNEF/extra-fine pitches.
//!
//! Values follow the usual references (ASME B18.2.8 / Machinery's Handbook
//! close and free fits, 75% tap drills, ISO 273 fine/medium/coarse clearance,
//! socket-head counterbores, flat-head countersinks). They are close to
//! Onshape's table but were not copied from it verbatim.
//!
//! [`ParamValue::Lookup`]: crate::params::ParamValue::Lookup

use crate::params::{ParamValue, ParamValues};
use noshape_units::ExprValue;

/// Dimensions resolved from a complete table path. Lengths in meters, angles
/// in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoleSpec {
    /// Main hole (shaft) diameter: drill size, clearance diameter or, for a
    /// tapped hole without fastener fit, the tap drill diameter. For external
    /// threads the major diameter.
    pub hole_diameter: f64,
    /// Tap drill diameter (tapped holes).
    pub tap_drill_diameter: Option<f64>,
    /// Thread major diameter (tapped holes, external threads).
    pub major_diameter: Option<f64>,
    /// Thread minor diameter (external threads).
    pub minor_diameter: Option<f64>,
    /// Thread pitch (tapped holes, external threads).
    pub pitch: Option<f64>,
    pub cbore_diameter: Option<f64>,
    pub cbore_depth: Option<f64>,
    pub csink_diameter: Option<f64>,
    pub csink_angle: Option<f64>,
}

impl HoleSpec {
    fn plain(d: f64) -> Self {
        Self {
            hole_diameter: d,
            tap_drill_diameter: None,
            major_diameter: None,
            minor_diameter: None,
            pitch: None,
            cbore_diameter: None,
            cbore_depth: None,
            csink_diameter: None,
            csink_angle: None,
        }
    }
    /// Whether the entry describes a threaded hole (FS `showTappedDepth`).
    pub fn is_threaded(&self) -> bool {
        self.pitch.is_some()
    }
}

/// (id, label) option of a level.
pub type Opt = (String, String);

/// Table names used by [`crate::params::ParamKind::Lookup`] parameters.
pub const TABLE_ANSI_HOLE: &str = "hole.ansi";
pub const TABLE_ANSI_HOLE_EX: &str = "hole.ansiEx";
pub const TABLE_ISO_HOLE: &str = "hole.iso";
pub const TABLE_ISO_HOLE_EX: &str = "hole.isoEx";
pub const TABLE_ANSI_THREAD_CLASS: &str = "thread.ansiClass";
pub const TABLE_ISO_THREAD_CLASS: &str = "thread.isoClass";
pub const TABLE_EXTERNAL_THREAD: &str = "externalThread";
/// Generic hole table whose first level is the standard ("ANSI" / "ISO").
pub const TABLE_HOLE: &str = "hole";

pub const ALL_TABLES: &[&str] = &[
    TABLE_HOLE,
    TABLE_ANSI_HOLE,
    TABLE_ANSI_HOLE_EX,
    TABLE_ISO_HOLE,
    TABLE_ISO_HOLE_EX,
    TABLE_ANSI_THREAD_CLASS,
    TABLE_ISO_THREAD_CLASS,
    TABLE_EXTERNAL_THREAD,
];

// ---------------------------------------------------------------------------
// Raw data
// ---------------------------------------------------------------------------

const IN: f64 = 0.0254;
const MM: f64 = 0.001;

/// One thread of a size: ANSI (threads per inch, series, tap drill [in]) or
/// ISO (pitch [mm], series, tap drill [mm]).
struct Thread {
    per: f64,
    series: &'static str,
    tap_drill: f64,
}

struct Size {
    name: &'static str,
    /// Major (nominal) diameter in the table's native unit.
    major: f64,
    threads: &'static [Thread],
    /// Clearance diameters, one per fit of the standard.
    clearance: &'static [f64],
    cbore: (f64, f64),
    csink: f64,
}

struct Standard {
    id: &'static str,
    unit: f64,
    sizes: &'static [Size],
    fits: &'static [&'static str],
    pitch_level: &'static str,
    default_size: &'static str,
    csink_angle_deg: f64,
    imperial: bool,
}

const fn t(per: f64, series: &'static str, tap_drill: f64) -> Thread {
    Thread { per, series, tap_drill }
}

static ANSI_SIZES: &[Size] = &[
    Size {
        name: "#0",
        major: 0.060,
        threads: &[t(80.0, "UNF", 0.0469)],
        clearance: &[0.0635, 0.0700, 0.064, 0.070, 0.076],
        cbore: (0.125, 0.060),
        csink: 0.119,
    },
    Size {
        name: "#1",
        major: 0.073,
        threads: &[t(64.0, "UNC", 0.0595), t(72.0, "UNF", 0.0595)],
        clearance: &[0.0760, 0.0810, 0.076, 0.081, 0.089],
        cbore: (0.156, 0.073),
        csink: 0.146,
    },
    Size {
        name: "#2",
        major: 0.086,
        threads: &[t(56.0, "UNC", 0.0700), t(64.0, "UNF", 0.0700)],
        clearance: &[0.0890, 0.0960, 0.089, 0.094, 0.104],
        cbore: (0.188, 0.086),
        csink: 0.172,
    },
    Size {
        name: "#3",
        major: 0.099,
        threads: &[t(48.0, "UNC", 0.0785), t(56.0, "UNF", 0.0820)],
        clearance: &[0.1040, 0.1100, 0.104, 0.110, 0.120],
        cbore: (0.219, 0.099),
        csink: 0.199,
    },
    Size {
        name: "#4",
        major: 0.112,
        threads: &[t(40.0, "UNC", 0.0890), t(48.0, "UNF", 0.0935)],
        clearance: &[0.1160, 0.1285, 0.116, 0.120, 0.128],
        cbore: (0.219, 0.112),
        csink: 0.225,
    },
    Size {
        name: "#5",
        major: 0.125,
        threads: &[t(40.0, "UNC", 0.1015), t(44.0, "UNF", 0.1040)],
        clearance: &[0.1285, 0.1360, 0.128, 0.136, 0.147],
        cbore: (0.250, 0.125),
        csink: 0.252,
    },
    Size {
        name: "#6",
        major: 0.138,
        threads: &[t(32.0, "UNC", 0.1065), t(40.0, "UNF", 0.1130)],
        clearance: &[0.1440, 0.1495, 0.144, 0.150, 0.161],
        cbore: (0.281, 0.138),
        csink: 0.279,
    },
    Size {
        name: "#8",
        major: 0.164,
        threads: &[t(32.0, "UNC", 0.1360), t(36.0, "UNF", 0.1360)],
        clearance: &[0.1695, 0.1770, 0.170, 0.177, 0.191],
        cbore: (0.312, 0.164),
        csink: 0.332,
    },
    Size {
        name: "#10",
        major: 0.190,
        threads: &[t(24.0, "UNC", 0.1495), t(32.0, "UNF", 0.1590)],
        clearance: &[0.1960, 0.2010, 0.196, 0.201, 0.221],
        cbore: (0.375, 0.190),
        csink: 0.385,
    },
    Size {
        name: "#12",
        major: 0.216,
        threads: &[t(24.0, "UNC", 0.1770), t(28.0, "UNF", 0.1820)],
        clearance: &[0.2210, 0.2280, 0.221, 0.228, 0.250],
        cbore: (0.406, 0.216),
        csink: 0.438,
    },
    Size {
        name: "1/4",
        major: 0.2500,
        threads: &[t(20.0, "UNC", 0.2010), t(28.0, "UNF", 0.2130)],
        clearance: &[0.2570, 0.2660, 0.257, 0.266, 0.281],
        cbore: (0.438, 0.250),
        csink: 0.507,
    },
    Size {
        name: "5/16",
        major: 0.3125,
        threads: &[t(18.0, "UNC", 0.2570), t(24.0, "UNF", 0.2720)],
        clearance: &[0.3230, 0.3320, 0.323, 0.332, 0.344],
        cbore: (0.531, 0.312),
        csink: 0.635,
    },
    Size {
        name: "3/8",
        major: 0.3750,
        threads: &[t(16.0, "UNC", 0.3125), t(24.0, "UNF", 0.3320)],
        clearance: &[0.3860, 0.3970, 0.386, 0.397, 0.406],
        cbore: (0.625, 0.375),
        csink: 0.762,
    },
    Size {
        name: "7/16",
        major: 0.4375,
        threads: &[t(14.0, "UNC", 0.3680), t(20.0, "UNF", 0.3906)],
        clearance: &[0.4531, 0.4688, 0.453, 0.469, 0.484],
        cbore: (0.719, 0.438),
        csink: 0.812,
    },
    Size {
        name: "1/2",
        major: 0.5000,
        threads: &[t(13.0, "UNC", 0.4219), t(20.0, "UNF", 0.4531)],
        clearance: &[0.5156, 0.5312, 0.516, 0.531, 0.562],
        cbore: (0.812, 0.500),
        csink: 0.875,
    },
    Size {
        name: "5/8",
        major: 0.6250,
        threads: &[t(11.0, "UNC", 0.5312), t(18.0, "UNF", 0.5781)],
        clearance: &[0.6406, 0.6562, 0.641, 0.656, 0.688],
        cbore: (1.000, 0.625),
        csink: 1.125,
    },
    Size {
        name: "3/4",
        major: 0.7500,
        threads: &[t(10.0, "UNC", 0.6562), t(16.0, "UNF", 0.6875)],
        clearance: &[0.7656, 0.7812, 0.766, 0.781, 0.812],
        cbore: (1.188, 0.750),
        csink: 1.375,
    },
    Size {
        name: "7/8",
        major: 0.8750,
        threads: &[t(9.0, "UNC", 0.7656), t(14.0, "UNF", 0.8125)],
        clearance: &[0.8906, 0.9062, 0.891, 0.906, 0.938],
        cbore: (1.375, 0.875),
        csink: 1.625,
    },
    Size {
        name: "1",
        major: 1.0000,
        threads: &[t(8.0, "UNC", 0.8750), t(12.0, "UNF", 0.9219)],
        clearance: &[1.0156, 1.0312, 1.016, 1.031, 1.062],
        cbore: (1.625, 1.000),
        csink: 1.875,
    },
];

/// ISO metric: tap drill = major - pitch (75% engagement rule).
static ISO_SIZES: &[Size] = &[
    Size { name: "M1.6", major: 1.6, threads: &[t(0.35, "Coarse", 1.25), t(0.2, "Fine", 1.4)], clearance: &[1.7, 1.8, 2.0], cbore: (3.5, 1.6), csink: 3.5 },
    Size { name: "M2", major: 2.0, threads: &[t(0.4, "Coarse", 1.6), t(0.25, "Fine", 1.75)], clearance: &[2.2, 2.4, 2.6], cbore: (4.4, 2.0), csink: 4.4 },
    Size { name: "M2.5", major: 2.5, threads: &[t(0.45, "Coarse", 2.05), t(0.35, "Fine", 2.15)], clearance: &[2.7, 2.9, 3.1], cbore: (5.5, 2.5), csink: 5.5 },
    Size { name: "M3", major: 3.0, threads: &[t(0.5, "Coarse", 2.5), t(0.35, "Fine", 2.65)], clearance: &[3.2, 3.4, 3.6], cbore: (6.5, 3.0), csink: 6.94 },
    Size { name: "M3.5", major: 3.5, threads: &[t(0.6, "Coarse", 2.9), t(0.35, "Fine", 3.15)], clearance: &[3.7, 3.9, 4.2], cbore: (7.0, 3.5), csink: 8.0 },
    Size { name: "M4", major: 4.0, threads: &[t(0.7, "Coarse", 3.3), t(0.5, "Fine", 3.5)], clearance: &[4.3, 4.5, 4.8], cbore: (8.0, 4.0), csink: 9.18 },
    Size { name: "M5", major: 5.0, threads: &[t(0.8, "Coarse", 4.2), t(0.5, "Fine", 4.5)], clearance: &[5.3, 5.5, 5.8], cbore: (10.0, 5.0), csink: 11.47 },
    Size { name: "M6", major: 6.0, threads: &[t(1.0, "Coarse", 5.0), t(0.75, "Fine", 5.25)], clearance: &[6.4, 6.6, 7.0], cbore: (11.0, 6.0), csink: 13.7 },
    Size {
        name: "M8",
        major: 8.0,
        threads: &[t(1.25, "Coarse", 6.8), t(1.0, "Fine", 7.0), t(0.75, "Fine", 7.25)],
        clearance: &[8.4, 9.0, 10.0],
        cbore: (15.0, 8.0),
        csink: 18.3,
    },
    Size {
        name: "M10",
        major: 10.0,
        threads: &[t(1.5, "Coarse", 8.5), t(1.25, "Fine", 8.75), t(1.0, "Fine", 9.0), t(0.75, "Fine", 9.25)],
        clearance: &[10.5, 11.0, 12.0],
        cbore: (18.0, 10.0),
        csink: 22.8,
    },
    Size {
        name: "M12",
        major: 12.0,
        threads: &[t(1.75, "Coarse", 10.25), t(1.5, "Fine", 10.5), t(1.25, "Fine", 10.75), t(1.0, "Fine", 11.0)],
        clearance: &[13.0, 13.5, 14.5],
        cbore: (20.0, 12.0),
        csink: 27.3,
    },
    Size {
        name: "M14",
        major: 14.0,
        threads: &[t(2.0, "Coarse", 12.0), t(1.5, "Fine", 12.5), t(1.25, "Fine", 12.75)],
        clearance: &[15.0, 15.5, 16.5],
        cbore: (24.0, 14.0),
        csink: 31.3,
    },
    Size {
        name: "M16",
        major: 16.0,
        threads: &[t(2.0, "Coarse", 14.0), t(1.5, "Fine", 14.5), t(1.0, "Fine", 15.0)],
        clearance: &[17.0, 17.5, 18.5],
        cbore: (26.0, 16.0),
        csink: 34.2,
    },
    Size {
        name: "M20",
        major: 20.0,
        threads: &[t(2.5, "Coarse", 17.5), t(2.0, "Fine", 18.0), t(1.5, "Fine", 18.5), t(1.0, "Fine", 19.0)],
        clearance: &[21.0, 22.0, 24.0],
        cbore: (33.0, 20.0),
        csink: 41.0,
    },
    Size {
        name: "M24",
        major: 24.0,
        threads: &[t(3.0, "Coarse", 21.0), t(2.0, "Fine", 22.0), t(1.5, "Fine", 22.5), t(1.0, "Fine", 23.0)],
        clearance: &[25.0, 26.0, 28.0],
        cbore: (40.0, 24.0),
        csink: 48.5,
    },
];

static ANSI: Standard = Standard {
    id: "ANSI",
    unit: IN,
    sizes: ANSI_SIZES,
    fits: &["Close", "Free", "Close (ASME)", "Normal (ASME)", "Loose (ASME)"],
    pitch_level: "Threads/inch",
    default_size: "1/4",
    csink_angle_deg: 82.0,
    imperial: true,
};

static ISO: Standard = Standard {
    id: "ISO",
    unit: MM,
    sizes: ISO_SIZES,
    fits: &["Close", "Normal", "Loose"],
    pitch_level: "Pitch",
    default_size: "M5",
    csink_angle_deg: 90.0,
    imperial: false,
};

const ANSI_THREAD_CLASSES: &[&str] = &["1B", "2B", "3B"];
const ISO_THREAD_CLASSES: &[&str] = &["4H", "5G", "5H", "6G", "6H", "7G", "7H", "8G", "8H"];

const HOLE_TYPES: &[&str] = &["Drilled", "Clearance", "Tapped"];
const TAP_TYPES: &[&str] = &["Straight tap"];
const FIT_NONE: &str = "None";

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn trim_num(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

fn thread_id(std: &Standard, th: &Thread) -> String {
    if std.imperial { format!("{} tpi", trim_num(th.per, 0)) } else { format!("{:.2} mm", th.per) }
}

fn thread_label(std: &Standard, th: &Thread) -> String {
    format!("{} ({})", thread_id(std, th), th.series)
}

fn thread_pitch(std: &Standard, th: &Thread) -> f64 {
    if std.imperial { IN / th.per } else { th.per * MM }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Drill sizes of a standard as (id/label, diameter in m), ascending.
fn drills(std: &Standard) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if std.imperial {
        const LETTERS: [(char, f64); 26] = [
            ('A', 0.234),
            ('B', 0.238),
            ('C', 0.242),
            ('D', 0.246),
            ('E', 0.250),
            ('F', 0.257),
            ('G', 0.261),
            ('H', 0.266),
            ('I', 0.272),
            ('J', 0.277),
            ('K', 0.281),
            ('L', 0.290),
            ('M', 0.295),
            ('N', 0.302),
            ('O', 0.316),
            ('P', 0.323),
            ('Q', 0.332),
            ('R', 0.339),
            ('S', 0.348),
            ('T', 0.358),
            ('U', 0.368),
            ('V', 0.377),
            ('W', 0.386),
            ('X', 0.397),
            ('Y', 0.404),
            ('Z', 0.413),
        ];
        for (c, d) in LETTERS {
            out.push((format!("{c} ({})", trim_num(d, 4)), d));
        }
        for n in 1..=64u32 {
            let g = gcd(n, 64);
            let frac = if n == 64 { "1".to_string() } else { format!("{}/{}", n / g, 64 / g) };
            let d = n as f64 / 64.0;
            out.push((format!("{frac} ({})", trim_num(d, 4)), d));
        }
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        out.into_iter().map(|(s, d)| (s, d * IN)).collect()
    } else {
        // 0.5 .. 10 mm by 0.1, 10.5 .. 20 by 0.5, 21 .. 38 by 1 (tenths of mm).
        let tenths = (5..=100).chain((105..=200).step_by(5)).chain((210..=380).step_by(10));
        for t in tenths {
            let d = t as f64 / 10.0;
            out.push((trim_num(d, 1), d * MM));
        }
        out
    }
}

fn default_drill(std: &Standard) -> String {
    if std.imperial { "1/4 (0.25)".into() } else { "5".into() }
}

fn opts(ids: &[&str]) -> Vec<Opt> {
    ids.iter().map(|s| (s.to_string(), s.to_string())).collect()
}

fn find_size<'a>(std: &'a Standard, name: &str) -> Option<&'a Size> {
    std.sizes.iter().find(|s| s.name == name)
}

fn find_thread<'a>(std: &Standard, size: &'a Size, id: &str) -> Option<&'a Thread> {
    size.threads.iter().find(|th| thread_id(std, th) == id)
}

// ---------------------------------------------------------------------------
// Cascade of one standard's hole table (path excludes the standard)
// ---------------------------------------------------------------------------

struct LevelInfo {
    name: &'static str,
    options: Vec<Opt>,
    default: String,
}

fn hole_level(std: &Standard, path: &[String]) -> Option<LevelInfo> {
    let p: Vec<&str> = path.iter().map(String::as_str).collect();
    let sizes = || std.sizes.iter().map(|s| (s.name.to_string(), s.name.to_string())).collect::<Vec<_>>();
    let fits = |with_none: bool| {
        let mut o = if with_none { opts(&[FIT_NONE]) } else { Vec::new() };
        o.extend(opts(std.fits));
        o
    };
    Some(match p.as_slice() {
        [] => LevelInfo { name: "Hole type", options: opts(HOLE_TYPES), default: HOLE_TYPES[0].into() },
        ["Drilled"] => LevelInfo { name: "Drill size", options: drills(std).into_iter().map(|(s, _)| (s.clone(), s)).collect(), default: default_drill(std) },
        ["Clearance"] => LevelInfo { name: "Size", options: sizes(), default: std.default_size.into() },
        ["Clearance", size] if find_size(std, size).is_some() => LevelInfo { name: "Fastener fit", options: fits(false), default: std.fits[0].into() },
        ["Tapped"] => LevelInfo { name: "Tap type", options: opts(TAP_TYPES), default: TAP_TYPES[0].into() },
        ["Tapped", "Straight tap"] => LevelInfo { name: "Size", options: sizes(), default: std.default_size.into() },
        ["Tapped", "Straight tap", size] => {
            let s = find_size(std, size)?;
            LevelInfo {
                name: std.pitch_level,
                options: s.threads.iter().map(|th| (thread_id(std, th), thread_label(std, th))).collect(),
                default: thread_id(std, &s.threads[0]),
            }
        }
        ["Tapped", "Straight tap", size, pitch] => {
            let s = find_size(std, size)?;
            find_thread(std, s, pitch)?;
            LevelInfo { name: "Fastener fit", options: fits(true), default: FIT_NONE.into() }
        }
        _ => return None,
    })
}

fn hole_values(std: &Standard, path: &[String]) -> Option<HoleSpec> {
    let p: Vec<&str> = path.iter().map(String::as_str).collect();
    let u = std.unit;
    let with_heads = |mut h: HoleSpec, s: &Size| {
        h.cbore_diameter = Some(s.cbore.0 * u);
        h.cbore_depth = Some(s.cbore.1 * u);
        h.csink_diameter = Some(s.csink * u);
        h.csink_angle = Some(std.csink_angle_deg.to_radians());
        h
    };
    let fit_index = |fit: &str| std.fits.iter().position(|f| *f == fit);
    match p.as_slice() {
        ["Drilled", drill] => drills(std).into_iter().find(|(s, _)| s == drill).map(|(_, d)| HoleSpec::plain(d)),
        ["Clearance", size, fit] => {
            let s = find_size(std, size)?;
            let d = s.clearance[fit_index(fit)?];
            Some(with_heads(HoleSpec::plain(d * u), s))
        }
        ["Tapped", "Straight tap", size, pitch, fit] => {
            let s = find_size(std, size)?;
            let th = find_thread(std, s, pitch)?;
            let tap = th.tap_drill * u;
            let hole = if *fit == FIT_NONE { tap } else { s.clearance[fit_index(fit)?] * u };
            let mut h = with_heads(HoleSpec::plain(hole), s);
            h.tap_drill_diameter = Some(tap);
            h.major_diameter = Some(s.major * u);
            h.pitch = Some(thread_pitch(std, th));
            Some(h)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// External threads: [standard, size, pitch]
// ---------------------------------------------------------------------------

fn standard(id: &str) -> Option<&'static Standard> {
    match id {
        "ANSI" => Some(&ANSI),
        "ISO" => Some(&ISO),
        _ => None,
    }
}

fn standards_level(default: &str) -> LevelInfo {
    LevelInfo { name: "Standard", options: opts(&["ANSI", "ISO"]), default: default.into() }
}

fn thread_level(path: &[String]) -> Option<LevelInfo> {
    let Some(std_id) = path.first() else { return Some(standards_level("ANSI")) };
    let std = standard(std_id)?;
    match &path[1..] {
        [] => Some(LevelInfo {
            name: "Size",
            options: std.sizes.iter().map(|s| (s.name.to_string(), s.name.to_string())).collect(),
            default: std.default_size.into(),
        }),
        [size] => {
            let s = find_size(std, size)?;
            Some(LevelInfo {
                name: std.pitch_level,
                // std externalThread tables list the bare pitch ("20 tpi",
                // live qa5 ADV8); tapped holes add the series.
                options: s.threads.iter().map(|th| (thread_id(std, th), thread_id(std, th))).collect(),
                default: thread_id(std, &s.threads[0]),
            })
        }
        _ => None,
    }
}

fn thread_values(path: &[String]) -> Option<HoleSpec> {
    let [std_id, size, pitch] = path else { return None };
    let std = standard(std_id)?;
    let s = find_size(std, size)?;
    let th = find_thread(std, s, pitch)?;
    let p = thread_pitch(std, th);
    let major = s.major * std.unit;
    // External minor diameter: UNR d - 1.19078 P, ISO d3 = d - 1.22687 P.
    let k = if std.imperial { 1.19078 } else { 1.22687 };
    let mut h = HoleSpec::plain(major);
    h.major_diameter = Some(major);
    h.minor_diameter = Some(major - k * p);
    h.pitch = Some(p);
    Some(h)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

fn split_std(path: &[String]) -> Option<(&'static Standard, &[String])> {
    let (first, rest) = path.split_first()?;
    Some((standard(first)?, rest))
}

/// A lookup table registered at run time (a FeatureScript "Lookup Table"
/// annotation): one level per node.
#[derive(Clone, Debug, PartialEq)]
pub struct DynLevel {
    /// The level's key in a `LookupTablePath` (`"standard"`).
    pub key: String,
    /// Shown name (`"Standard"`).
    pub name: &'static str,
    pub default: String,
    /// (option, its sub-level; `None` for a leaf).
    pub entries: Vec<(String, Option<std::sync::Arc<DynLevel>>)>,
}

static DYNAMIC_TABLES: std::sync::LazyLock<parking_lot::RwLock<std::collections::HashMap<&'static str, std::sync::Arc<DynLevel>>>> = std::sync::LazyLock::new(Default::default);

/// Register (or replace) a run-time lookup table under `name`.
pub fn register_dynamic_table(name: &'static str, root: DynLevel) {
    DYNAMIC_TABLES.write().insert(name, std::sync::Arc::new(root));
}

/// The level reached by `path` in a run-time table.
pub fn dynamic_level(table: &str, path: &[String]) -> Option<std::sync::Arc<DynLevel>> {
    let mut level = DYNAMIC_TABLES.read().get(table)?.clone();
    for key in path {
        let next = level.entries.iter().find(|(k, _)| k == key)?.1.clone()?;
        level = next;
    }
    Some(level)
}

fn level_info(table: &str, path: &[String]) -> Option<LevelInfo> {
    if let Some(l) = dynamic_level(table, path) {
        return Some(LevelInfo { name: l.name, options: l.entries.iter().map(|(k, _)| (k.clone(), k.clone())).collect(), default: l.default.clone() });
    }
    match table {
        TABLE_HOLE => match split_std(path) {
            Some((std, rest)) => hole_level(std, rest),
            None if path.is_empty() => Some(standards_level("ANSI")),
            None => None,
        },
        TABLE_ANSI_HOLE | TABLE_ANSI_HOLE_EX => hole_level(&ANSI, path),
        TABLE_ISO_HOLE | TABLE_ISO_HOLE_EX => hole_level(&ISO, path),
        TABLE_ANSI_THREAD_CLASS if path.is_empty() => Some(LevelInfo { name: "Thread class", options: opts(ANSI_THREAD_CLASSES), default: "2B".into() }),
        TABLE_ISO_THREAD_CLASS if path.is_empty() => Some(LevelInfo { name: "Thread class", options: opts(ISO_THREAD_CLASSES), default: "6H".into() }),
        TABLE_EXTERNAL_THREAD => thread_level(path),
        _ => None,
    }
}

/// Options of the level that follows `path` in the generic hole table
/// (`path[0]` is the standard, "ANSI" or "ISO"). Empty when `path` is a leaf
/// or invalid.
pub fn levels(path: &[String]) -> Vec<Opt> {
    table_levels(TABLE_HOLE, path)
}

/// Leaf values of a complete generic hole-table path (`path[0]` = standard).
pub fn values(path: &[String]) -> Option<HoleSpec> {
    table_values(TABLE_HOLE, path)
}

/// Options of the level following `path` in the named table.
pub fn table_levels(table: &str, path: &[String]) -> Vec<Opt> {
    level_info(table, path).map(|l| l.options).unwrap_or_default()
}

/// Display name of the level following `path` ("Hole type", "Size", ...).
pub fn table_level_name(table: &str, path: &[String]) -> Option<&'static str> {
    level_info(table, path).map(|l| l.name)
}

/// Default option of the level following `path`.
pub fn table_level_default(table: &str, path: &[String]) -> Option<String> {
    level_info(table, path).map(|l| l.default)
}

/// Leaf values of a complete path in the named table. Thread-class tables
/// have no dimensional values and always return `None`.
pub fn table_values(table: &str, path: &[String]) -> Option<HoleSpec> {
    match table {
        TABLE_HOLE => {
            let (std, rest) = split_std(path)?;
            hole_values(std, rest)
        }
        TABLE_ANSI_HOLE | TABLE_ANSI_HOLE_EX => hole_values(&ANSI, path),
        TABLE_ISO_HOLE | TABLE_ISO_HOLE_EX => hole_values(&ISO, path),
        TABLE_EXTERNAL_THREAD => thread_values(path),
        _ => None,
    }
}

/// Initialise a new Hole like Onshape: the default Standard path's values
/// fill the numeric fields, so the dialog's drill size and diameter agree
/// from the start. Units keep hole.fs's default, Inch, whatever the
/// workspace units (live: a mm document opens a new Hole on the Inch tab).
pub fn init_new_hole(params: &mut ParamValues) {
    sync_hole_standard(params);
    // The fields now hold the table values, but the hole is not marked as
    // synced: until the user edits it, a Standard path set later (by code, or
    // by a dialog that skips the edit logic) still takes the table's values.
    params.set(TABLE_SYNCED_PARAM, ParamValue::Bool(false));
}

/// Hole edit logic after the units system or style changed: the Standard
/// that becomes visible gets its default path if it has none, and its values
/// are copied into the numeric fields (so switching Metric to Inch does not
/// leave a metric diameter behind). Returns whether values were applied.
pub fn sync_hole_standard(params: &mut ParamValues) -> bool {
    let (pid, table) = hole_table_param(params);
    if params.lookup(pid).is_empty() {
        params.set(pid, ParamValue::Lookup(default_path(table)));
    }
    apply_table_to_params(params)
}

/// Path obtained by following every level's default from the top.
pub fn default_path(table: &str) -> Vec<String> {
    let mut path = Vec::new();
    while let Some(d) = table_level_default(table, &path) {
        path.push(d);
        if path.len() > 16 {
            break;
        }
    }
    path
}

/// Whether `path` is a complete, valid path of `table`.
pub fn is_complete(table: &str, path: &[String]) -> bool {
    match table {
        TABLE_ANSI_THREAD_CLASS | TABLE_ISO_THREAD_CLASS => path.len() == 1 && level_info(table, &[]).is_some_and(|l| l.options.iter().any(|o| o.0 == path[0])),
        _ => table_values(table, path).is_some(),
    }
}

/// Thread standard implied by a hole path (FS `threadStandard`): "ANSI" or
/// "ISO" for straight-tapped holes, "UNSET" otherwise.
pub fn thread_standard(table: &str, path: &[String]) -> &'static str {
    let tapped = path.first().map(String::as_str) == Some("Tapped") || path.get(1).map(String::as_str) == Some("Tapped");
    if !tapped {
        return "UNSET";
    }
    match table {
        TABLE_ANSI_HOLE | TABLE_ANSI_HOLE_EX => "ANSI",
        TABLE_ISO_HOLE | TABLE_ISO_HOLE_EX => "ISO",
        TABLE_HOLE => match path.first().map(String::as_str) {
            Some("ANSI") => "ANSI",
            Some("ISO") => "ISO",
            _ => "UNSET",
        },
        _ => "UNSET",
    }
}

// ---------------------------------------------------------------------------
// Hole dialog edit logic
// ---------------------------------------------------------------------------

/// Numeric Hole parameters driven by the Standard table.
pub const HOLE_TABLE_DIMENSIONS: &[&str] = &["holeDiameterV3", "tapDrillDiameterV3", "cBoreDiameterV3", "cBoreDepthV3", "cSinkDiameterV3", "cSinkAngleV3"];

/// Hidden Hole parameter set by [`apply_table_to_params`]: the numeric
/// parameters hold the table values (or the user's later edits) and the
/// evaluator uses them as they are.
pub const TABLE_SYNCED_PARAM: &str = "tableSynced";

/// The Standard lookup parameter of a Hole that is visible for its units and
/// style, as (parameter id, table name).
pub fn hole_table_param(params: &ParamValues) -> (&'static str, &'static str) {
    match (params.is("unitsSystem", "INCH"), params.is("styleV2", "SIMPLE")) {
        (true, true) => ("ansiHoleTableEx", TABLE_ANSI_HOLE_EX),
        (true, false) => ("ansiHoleTable", TABLE_ANSI_HOLE),
        (false, true) => ("isoHoleTableEx", TABLE_ISO_HOLE_EX),
        (false, false) => ("isoHoleTable", TABLE_ISO_HOLE),
    }
}

/// Complete a (possibly partial) table path with each level's default.
pub fn complete_path(table: &str, path: &[String]) -> Vec<String> {
    let mut p = path.to_vec();
    while let Some(d) = table_level_default(table, &p) {
        p.push(d);
        if p.len() > 16 {
            break;
        }
    }
    p
}

fn fmt_num(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Hole edit logic (Onshape's behavior when the Standard changes): copy the
/// table values of the Hole's current Standard path into its numeric
/// parameters, so the dialog shows them and later edits start from them.
///
/// Call it from the Hole dialog whenever the visible Standard lookup
/// (see [`hole_table_param`]), the units system or the style changes; do not
/// call it when the user edits a numeric parameter (edits win until the next
/// Standard change). Sets: `holeDiameterV3`, `tapDrillDiameterV3`,
/// `cBoreDiameterV3`, `cBoreDepthV3`, `cSinkDiameterV3`, `cSinkAngleV3`
/// (as literal expressions in mm / in and degrees), the hidden
/// `majorDiameter`, `threadStandard`, `showTappedDepth`, `hasClearance` and
/// [`TABLE_SYNCED_PARAM`]. Returns false (and changes nothing) when no
/// Standard is selected or its path has no values.
pub fn apply_table_to_params(params: &mut ParamValues) -> bool {
    let (pid, table) = hole_table_param(params);
    let raw = params.lookup(pid).to_vec();
    if raw.is_empty() {
        return false;
    }
    let path = complete_path(table, &raw);
    let Some(spec) = table_values(table, &path) else { return false };
    let inch = params.is("unitsSystem", "INCH");
    let len = |m: f64| ParamValue::Expr(ExprValue::new(if inch { format!("{} in", fmt_num(m / IN, 5)) } else { format!("{} mm", fmt_num(m * 1000.0, 4)) }));
    params.set("holeDiameterV3", len(spec.hole_diameter));
    if let Some(td) = spec.tap_drill_diameter {
        params.set("tapDrillDiameterV3", len(td));
    }
    if let Some(v) = spec.cbore_diameter {
        params.set("cBoreDiameterV3", len(v));
    }
    if let Some(v) = spec.cbore_depth {
        params.set("cBoreDepthV3", len(v));
    }
    if let Some(v) = spec.csink_diameter {
        params.set("cSinkDiameterV3", len(v));
    }
    if let Some(a) = spec.csink_angle {
        params.set("cSinkAngleV3", ParamValue::Expr(ExprValue::new(format!("{} deg", fmt_num(a.to_degrees(), 4)))));
    }
    if let Some(v) = spec.major_diameter {
        params.set("majorDiameter", len(v));
    }
    params.set("threadStandard", ParamValue::Enum(thread_standard(table, &path).into()));
    params.set("showTappedDepth", ParamValue::Bool(spec.is_threaded()));
    params.set("hasClearance", ParamValue::Bool(spec.tap_drill_diameter.is_some_and(|t| (t - spec.hole_diameter).abs() > 1e-12)));
    params.set(TABLE_SYNCED_PARAM, ParamValue::Bool(true));
    true
}

/// Size whose major diameter matches `diameter` (m) within `tol` (external
/// thread auto-sizing). Returns the default path for that size.
/// Onshape's hole edit logic for a blind tapped hole (live L49/L50): Depth,
/// Tapped depth and Tap clearance stay linked, Depth = Tapped depth + Tap
/// clearance × pitch. A new size sets the clearance to 0 (Depth = Tapped
/// depth); a new Depth moves the clearance (or the tapped depth when the
/// Depth is less than it); a new clearance moves the tapped depth; a new
/// tapped depth moves the Depth. Returns the parameters to change.
pub fn link_tapped_depths(prev: &ParamValues, now: &ParamValues, units: &noshape_units::UnitPrefs) -> Vec<(&'static str, ParamValue)> {
    use noshape_units::{Dim, Quantity};
    if !now.bool("showTappedDepth") || !now.is("endStyleV2", "BLIND") {
        return Vec::new();
    }
    let (pid, table) = hole_table_param(now);
    let Some(pitch) = table_values(table, &complete_path(table, now.lookup(pid))).and_then(|s| s.pitch).filter(|p| *p > 0.0) else { return Vec::new() };
    let empty = std::collections::BTreeMap::<String, Quantity>::new();
    let val = |v: &ParamValues, id: &str, dim: Dim| -> Option<f64> {
        let e = v.expr(id)?;
        e.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&e.text, dim, units, &empty).ok().map(|q| q.value))
    };
    let (Some(depth), Some(tapped), Some(clear)) = (val(now, "holeDepthV3", Dim::LENGTH), val(now, "tappedDepthV3", Dim::LENGTH), val(now, "tapClearance", Dim::UNITLESS)) else { return Vec::new() };
    let changed = |id: &str, dim: Dim| match (val(prev, id, dim), val(now, id, dim)) {
        (Some(a), Some(b)) => (a - b).abs() > 1e-12,
        _ => false,
    };
    let len = |m: f64| ParamValue::Expr(ExprValue::new(noshape_units::format_quantity(Quantity::new(m.max(0.0), Dim::LENGTH), units)));
    let count = |c: f64| ParamValue::Expr(ExprValue::new(noshape_units::format_number(c.max(0.0), 6)));
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    if prev.lookup(pid) != now.lookup(pid) {
        let mut out = Vec::new();
        if !close(clear, 0.0) {
            out.push(("tapClearance", count(0.0)));
        }
        if !close(depth, tapped) {
            out.push(("holeDepthV3", len(tapped)));
        }
        return out;
    }
    if changed("holeDepthV3", Dim::LENGTH) {
        return if depth >= tapped { vec![("tapClearance", count((depth - tapped) / pitch))] } else { vec![("tappedDepthV3", len(depth - clear * pitch))] };
    }
    if changed("tapClearance", Dim::UNITLESS) {
        return vec![("tappedDepthV3", len(depth - clear * pitch))];
    }
    if changed("tappedDepthV3", Dim::LENGTH) {
        return vec![("holeDepthV3", len(tapped + clear * pitch))];
    }
    Vec::new()
}

pub fn external_thread_for_diameter(standard_id: &str, diameter: f64, tol: f64) -> Option<Vec<String>> {
    let std = standard(standard_id)?;
    let s = std.sizes.iter().find(|s| (s.major * std.unit - diameter).abs() <= tol)?;
    Some(vec![std.id.to_string(), s.name.to_string(), thread_id(std, &s.threads[0])])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn default_paths_are_complete() {
        for t in ALL_TABLES {
            let path = default_path(t);
            assert!(!path.is_empty(), "{t}");
            assert!(is_complete(t, &path), "{t}: {path:?}");
        }
        assert_eq!(default_path(TABLE_ANSI_HOLE), p(&["Drilled", "1/4 (0.25)"]));
        assert_eq!(default_path(TABLE_EXTERNAL_THREAD), p(&["ANSI", "1/4", "20 tpi"]));
    }

    #[test]
    fn every_level_default_is_an_option() {
        // Walk every path of every table (bounded) and check defaults.
        fn walk(table: &str, path: &mut Vec<String>, budget: &mut usize) {
            let Some(info) = level_info(table, path) else {
                assert!(is_complete(table, path), "{table}: dead end {path:?}");
                return;
            };
            assert!(info.options.iter().any(|o| o.0 == info.default), "{table} {path:?}: default {}", info.default);
            for (id, _) in info.options {
                if *budget == 0 {
                    return;
                }
                *budget -= 1;
                path.push(id);
                walk(table, path, budget);
                path.pop();
            }
        }
        for t in ALL_TABLES {
            let mut budget = 20_000;
            walk(t, &mut Vec::new(), &mut budget);
        }
    }

    #[test]
    fn table_values_copied_into_hole_params() {
        let mut v = ParamValues::default();
        v.set("unitsSystem", ParamValue::Enum("METRIC".into()));
        v.set("styleV2", ParamValue::Enum("C_BORE".into()));
        assert!(!apply_table_to_params(&mut v), "no standard selected");
        v.set("isoHoleTable", ParamValue::Lookup(p(&["Tapped", "Straight tap", "M5"])));
        assert!(apply_table_to_params(&mut v));
        assert_eq!(v.expr("holeDiameterV3").unwrap().text, "4.2 mm");
        assert_eq!(v.expr("tapDrillDiameterV3").unwrap().text, "4.2 mm");
        assert_eq!(v.expr("cBoreDiameterV3").unwrap().text, "10 mm");
        assert_eq!(v.enum_("threadStandard"), "ISO");
        assert!(v.bool("showTappedDepth") && !v.bool("hasClearance") && v.bool(TABLE_SYNCED_PARAM));
        // Inch clearance hole with a fit.
        let mut v = ParamValues::default();
        v.set("unitsSystem", ParamValue::Enum("INCH".into()));
        v.set("styleV2", ParamValue::Enum("SIMPLE".into()));
        v.set("ansiHoleTableEx", ParamValue::Lookup(p(&["Tapped", "Straight tap", "1/4", "20 tpi", "Close"])));
        assert!(apply_table_to_params(&mut v));
        assert_eq!(v.expr("holeDiameterV3").unwrap().text, "0.257 in");
        assert_eq!(v.expr("tapDrillDiameterV3").unwrap().text, "0.201 in");
        assert!(v.bool("hasClearance"));
    }

    #[test]
    fn known_values() {
        let m5 = values(&p(&["ISO", "Tapped", "Straight tap", "M5", "0.80 mm", "None"])).unwrap();
        assert!((m5.hole_diameter - 4.2e-3).abs() < 1e-9);
        assert!((m5.pitch.unwrap() - 0.8e-3).abs() < 1e-12);
        assert!((m5.cbore_diameter.unwrap() - 10e-3).abs() < 1e-9);
        let q = values(&p(&["ANSI", "Tapped", "Straight tap", "1/4", "20 tpi", "Close"])).unwrap();
        assert!((q.hole_diameter - 0.257 * IN).abs() < 1e-9);
        assert!((q.tap_drill_diameter.unwrap() - 0.201 * IN).abs() < 1e-9);
        assert_eq!(thread_standard(TABLE_ANSI_HOLE, &p(&["Tapped", "Straight tap"])), "ANSI");
        assert_eq!(thread_standard(TABLE_ISO_HOLE, &p(&["Clearance", "M5"])), "UNSET");
        let ext = external_thread_for_diameter("ISO", 8e-3, 1e-6).unwrap();
        assert_eq!(ext, p(&["ISO", "M8", "1.25 mm"]));
        assert!(table_values(TABLE_EXTERNAL_THREAD, &ext).unwrap().minor_diameter.unwrap() < 8e-3);
    }
}
