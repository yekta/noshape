//! The standard content catalogue: every Standard › Category › Class ›
//! Component of Onshape's library with its parameters, sizes, lengths and
//! dimensions, loaded once from `data/*.json`.
//!
//! The JSON files are generated (`data/README.md`): the option lists
//! (classes, components, sizes, lengths, other selects and their defaults)
//! are live's, the dimensions come from the published standard tables, and
//! each component names its source.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// Geometry family of a component (one script feature each, `fasteners.fs`;
/// the component's `form` picks the head, drive, point… within it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Shape {
    Screw,
    Nut,
    Washer,
    Pin,
    Key,
    RetainingRing,
    ORing,
}

impl Shape {
    /// The script feature building it.
    pub fn feature(self) -> &'static str {
        match self {
            Shape::Screw => "screw",
            Shape::Nut => "nut",
            Shape::Washer => "washer",
            Shape::Pin => "pin",
            Shape::Key => "key",
            Shape::RetainingRing => "retainingRing",
            Shape::ORing => "oRing",
        }
    }
    /// Whether the component has a length (bolts, screws, pins, keys).
    pub fn has_length(self) -> bool {
        matches!(self, Shape::Screw | Shape::Pin | Shape::Key)
    }
    /// Nuts, washers, rings fit over a shaft: auto-size rounds up.
    pub fn fits_over(self) -> bool {
        matches!(self, Shape::Nut | Shape::Washer | Shape::RetainingRing | Shape::ORing)
    }
    /// The script's dimension giving the part's height (its top face, where
    /// the second mate connector sits), for parts with no length.
    fn height_key(self) -> &'static [&'static str] {
        match self {
            Shape::Nut => &["m"],
            Shape::Washer => &["h"],
            Shape::RetainingRing => &["s"],
            Shape::ORing => &["d2"],
            _ => &[],
        }
    }
}

/// A select (or checkbox) of the parameter group other than Size, Length,
/// Thread length, Material, Finish, Part number and Description: "Series",
/// "Drive type", "Bearing face", "Head slot", "Head type"…, in live's order.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Param {
    pub label: String,
    pub options: Vec<String>,
    pub default: String,
    /// Shown as a checkbox (options "checked" / "unchecked").
    #[serde(default)]
    pub checkbox: bool,
    /// The script values an option changes (e.g. Head slot checked: the
    /// slot drive), over the component's `form`.
    #[serde(default)]
    pub forms: BTreeMap<String, BTreeMap<String, f64>>,
}

/// One size row.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    pub name: String,
    /// The row applies when these parameters have these values (e.g. the
    /// Wide series of a washer): rows of one name differ only here.
    #[serde(default)]
    pub when: BTreeMap<String, String>,
    /// The part name without the length, as live writes it
    /// ("Hex socket head cap screw M6x1.00", "Type A flat washer 3/8 Narrow").
    pub label: String,
    /// Nominal (bolt) diameter in the component's unit.
    pub d: f64,
    /// The script's dimensions in the component's unit, keyed by the
    /// standard's symbols (dk, k, s, t, m, d1, d2, h…).
    pub dims: BTreeMap<String, f64>,
    /// Live's Length list for this size (bare numbers in the component's
    /// unit, live's order).
    #[serde(default)]
    pub lengths: Vec<String>,
    /// The script's counts for this size (ribs, teeth), plain numbers.
    #[serde(default)]
    pub counts: BTreeMap<String, f64>,
    /// Live's Thread length options by length (the first is the default).
    #[serde(default)]
    pub threads: BTreeMap<String, Vec<String>>,
    /// Selects whose options depend on the size (live's lists for it,
    /// the first the default), by label.
    #[serde(default)]
    pub values: BTreeMap<String, Vec<String>>,
}

/// How the Thread length list follows from the length.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "rule")]
pub enum ThreadRule {
    /// ASME B18.2.1: L_T = 2D + 1/4 in (L ≤ 6 in), 2D + 1/2 in above.
    Asme,
    /// ISO 4014 / 888: b = 2d + 6 (l ≤ 125), 2d + 12 (≤ 200), 2d + 25.
    Iso,
}

/// A component of the catalogue.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    #[serde(skip)]
    pub standard: String,
    pub category: String,
    pub class: String,
    pub name: String,
    pub shape: Shape,
    /// The script's forms (head, drive, neck, point, form…) and fixed
    /// values (a taper, a tooth count) for every size.
    #[serde(default)]
    pub form: BTreeMap<String, f64>,
    #[serde(skip)]
    pub inch: bool,
    /// The size select's label when not "Size" (live).
    #[serde(default)]
    pub size_label: Option<String>,
    /// The length select's label when not "Length" (live: "Shoulder length").
    #[serde(default)]
    pub length_label: Option<String>,
    /// Material and Finish options when not the usual (live), the first
    /// the default.
    #[serde(default)]
    pub materials: Option<Vec<String>>,
    #[serde(default)]
    pub finishes: Option<Vec<String>>,
    #[serde(default)]
    pub params: Vec<Param>,
    pub sizes: Vec<Size>,
    /// The size it opens on when not the first (live: the ANSI Type A
    /// washer opens on 3/8).
    #[serde(default)]
    pub default_size: Option<String>,
    #[serde(default)]
    pub thread: Option<ThreadRule>,
    /// Where the dimensions come from (standard, edition, table, pages).
    pub source: String,
}

#[derive(Deserialize)]
struct File {
    standard: String,
    #[serde(default)]
    inch: bool,
    components: Vec<Component>,
}

const FILES: [&str; 6] = [
    include_str!("data/ansi-inch.json"),
    include_str!("data/din.json"),
    include_str!("data/iso.json"),
    include_str!("data/nas.json"),
    include_str!("data/pem.json"),
    include_str!("data/sae.json"),
];

/// Every component, in file order.
pub static CATALOGUE: LazyLock<Vec<Component>> = LazyLock::new(|| {
    let mut out = Vec::new();
    for text in FILES {
        let files: Vec<File> = serde_json::from_str(text).expect("standard content catalogue");
        for f in files {
            for mut c in f.components {
                c.standard = f.standard.clone();
                c.inch = f.inch;
                out.push(c);
            }
        }
    }
    out
});

impl Component {
    /// Millimetres per unit of the component's tables.
    pub fn unit(&self) -> f64 {
        if self.inch { 25.4 } else { 1.0 }
    }

    /// The first row of a size.
    pub fn size(&self, name: &str) -> Option<&Size> {
        self.sizes.iter().find(|s| s.name == name)
    }

    /// Size names in live's order (rows differing by parameters once).
    pub fn size_names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.sizes {
            if !out.contains(&s.name.as_str()) {
                out.push(&s.name);
            }
        }
        out
    }

    /// The size a component opens on (live: the first, or `default_size`).
    pub fn default_size(&self) -> &Size {
        self.default_size.as_deref().and_then(|n| self.size(n)).unwrap_or(&self.sizes[0])
    }

    /// Lengths offered for a size (live's list for it).
    pub fn lengths(&self, size: &Size) -> Vec<String> {
        if !self.shape.has_length() {
            return Vec::new();
        }
        size.lengths.clone()
    }

    /// Default length for a size: the first offered (live).
    pub fn default_length(&self, size: &Size) -> Option<String> {
        self.lengths(size).first().cloned()
    }

    /// A length name in meters: a bare number in the component's unit
    /// ("0.25", "16"), or "20 mm" / "1 1/4 in".
    pub fn length_m(&self, name: &str) -> Option<f64> {
        super::length_value(name).or_else(|| name.trim().parse::<f64>().ok().map(|v| v * self.unit() * 1e-3))
    }

    /// Nominal diameter in meters.
    pub fn diameter_m(&self, size: &Size) -> f64 {
        size.d * self.unit() * 1e-3
    }

    /// Height of a nut, ring or O-ring or thickness of a washer in the
    /// component's unit (where its top mate connector sits).
    pub fn thickness(&self, size: &Size) -> f64 {
        self.shape.height_key().iter().find_map(|k| size.dims.get(*k).copied()).unwrap_or(0.0)
    }

    /// The size select's label (live: "Size" unless the component names it).
    pub fn size_label(&self) -> &str {
        self.size_label.as_deref().unwrap_or("Size")
    }

    /// The length select's label (live: "Length" unless the component names it).
    pub fn length_label(&self) -> &str {
        self.length_label.as_deref().unwrap_or("Length")
    }

    /// Material options (the first is the default).
    pub fn materials(&self) -> Vec<&str> {
        match &self.materials {
            Some(m) => m.iter().map(String::as_str).collect(),
            None => super::MATERIALS.to_vec(),
        }
    }

    /// Finish options (the first is the default).
    pub fn finishes(&self) -> Vec<&str> {
        match &self.finishes {
            Some(f) => f.iter().map(String::as_str).collect(),
            None => super::FINISHES.to_vec(),
        }
    }

    /// A select's options for a size (its own list when it depends on the size).
    pub fn param_options<'a>(&'a self, p: &'a Param, size: Option<&'a Size>) -> &'a [String] {
        size.and_then(|s| s.values.get(&p.label)).map(Vec::as_slice).unwrap_or(&p.options)
    }

    /// The Thread length options for a size and length (bolts and
    /// screws): live's list for it, else the length itself (fully
    /// threaded) and, when shorter, the standard's thread length.
    pub fn thread_lengths(&self, size: &Size, length: &str) -> Vec<String> {
        if let Some(t) = size.threads.get(length) {
            return t.clone();
        }
        let Some(l) = self.length_m(length).map(|l| l * 1e3 / self.unit()) else { return Vec::new() };
        let rule = self.thread.map(|rule| match rule {
            ThreadRule::Asme => 2.0 * size.d + if l <= 6.0 + 1e-9 { 0.25 } else { 0.5 },
            ThreadRule::Iso => 2.0 * size.d + if l <= 125.0 + 1e-9 { 6.0 } else if l <= 200.0 + 1e-9 { 12.0 } else { 25.0 },
        });
        let Some(t) = rule else { return Vec::new() };
        let mut out = vec![length.to_string()];
        if t < l - 1e-9 {
            out.push(format!("{}", (t * 1e4).round() / 1e4));
        }
        out
    }
}
