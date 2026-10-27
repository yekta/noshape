//! The user's preference files (per user, JSON in noshape's config
//! directory), shared by the app and the agent API: Graphics preferences,
//! the Part Studio toolbar (Customize toolbar…, the S shortcut toolbars).

use noshape_scene::GraphicsPrefs;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

fn graphics_path() -> std::path::PathBuf {
    noshape_scene::config_dir().join("graphics.json")
}

/// The saved Graphics preferences (Onshape's defaults when none).
pub fn load_graphics() -> GraphicsPrefs {
    std::fs::read(graphics_path()).ok().and_then(|b| serde_json::from_slice::<GraphicsPrefs>(&b).ok()).unwrap_or_default()
}

/// Save the Graphics preferences.
pub fn save_graphics(prefs: &GraphicsPrefs) {
    if let Ok(json) = serde_json::to_vec_pretty(prefs) {
        let _ = std::fs::write(graphics_path(), json);
    }
}

/// The Graphics preferences dialog's number fields and their ranges
/// (crates/app-core/src/graphics_prefs_dialog.rs rows).
const NUMBERS: [(&str, f64, f64); 9] = [
    ("ambient_occlusion", 0.0, 1.0),
    ("fov_deg", 20.0, 120.0),
    ("line_width", 0.1, 3.0),
    ("shininess", 0.01, 1.0),
    ("phantom_width", 0.1, 3.0),
    ("mesh_line_width", 0.1, 3.0),
    ("sketch_line_width", 0.1, 3.0),
    ("inactive_line_width", 0.1, 3.0),
    ("construction", 0.0, 50.0),
];

const COLORS: [&str; 7] = ["specular", "section_interference", "phantom_color", "sketch_constrained", "sketch_under", "sketch_under_occluded", "inactive_color"];

/// The preferences as the agent API shows them (colors as "#rrggbb").
pub fn graphics_json(p: &GraphicsPrefs) -> Value {
    let mut v = serde_json::to_value(p).unwrap_or(Value::Null);
    if let Some(m) = v.as_object_mut() {
        for c in COLORS {
            if let Some(n) = m.get(c).and_then(Value::as_u64) {
                m.insert(c.into(), json!(format!("#{n:06x}")));
            }
        }
    }
    v
}

/// `p` with the fields of `set` changed (numbers within the dialog's
/// ranges, colors "#rrggbb"); Err(field, why).
pub fn graphics_with(p: &GraphicsPrefs, set: &Map<String, Value>) -> Result<GraphicsPrefs, (String, String)> {
    let mut v = serde_json::to_value(p).map_err(|e| (String::new(), e.to_string()))?;
    let m = v.as_object_mut().expect("object");
    for (k, x) in set {
        if !m.contains_key(k) {
            return Err((k.clone(), format!("not a graphics preference (they are: {})", m.keys().cloned().collect::<Vec<_>>().join(", "))));
        }
        if COLORS.contains(&k.as_str()) {
            let hex = x.as_str().and_then(|s| s.strip_prefix('#')).filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok()).ok_or_else(|| (k.clone(), "a color \"#rrggbb\"".to_string()))?;
            m.insert(k.clone(), json!(hex));
            continue;
        }
        let (_, lo, hi) = NUMBERS.iter().find(|n| n.0 == k).copied().expect("every other field is a number");
        let ok = |n: &Value| n.as_f64().is_some_and(|f| (lo..=hi).contains(&f));
        let good = match x {
            Value::Array(a) => k == "construction" && a.len() == 4 && a.iter().all(ok) && a[0].as_f64() >= Some(1.0) && a[2].as_f64() >= Some(1.0),
            n => k != "construction" && ok(n),
        };
        if !good {
            let what = if k == "construction" { "[first dash, first gap, second dash, second gap] in px, dashes 1..50, gaps 0..50".to_string() } else { format!("a number from {lo} to {hi}") };
            return Err((k.clone(), what));
        }
        m.insert(k.clone(), x.clone());
    }
    serde_json::from_value(v).map_err(|e| (String::new(), e.to_string()))
}

// ------------------------------------------------------------ toolbar

/// Default S shortcut toolbar (Part Studio, live: two rows of five).
pub const DEFAULT_SHORTCUT_TOOLS: &[&str] = &["sketch", "extrude", "revolve", "sweep", "loft", "fillet", "chamfer", "hole", "booleanBodies", "transform"];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolbarPrefs {
    /// Tool ids removed from the Part Studio toolbar.
    #[serde(default)]
    pub removed: Vec<String>,
    /// Tool set order: indices into the default toolbar groups, ⊞ last
    /// (Customize toolbar…; `toolbar_customize::full_order`).
    #[serde(default)]
    pub group_order: Vec<usize>,
    /// Right-click the toolbar › Hide "Search tools".
    #[serde(default)]
    pub hide_search: bool,
    /// S shortcut toolbar tools (None = defaults).
    #[serde(default)]
    pub shortcut_tools: Option<Vec<String>>,
    /// S shortcut toolbar tools in a sketch, by tool name (None = defaults;
    /// Preferences › Shortcut toolbars › Sketch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch_shortcut_tools: Option<Vec<String>>,
    /// The drawing S toolbar's tools, by name (None = defaults; Preferences ›
    /// Shortcut toolbars › Drawing; `drawing_keys::SHORTCUT_CATALOGUE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing_shortcut_tools: Option<Vec<String>>,
}

impl ToolbarPrefs {
    fn path() -> std::path::PathBuf {
        noshape_scene::config_dir().join("part_studio_toolbar.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }

    pub fn shortcut_tools(&self) -> Vec<String> {
        self.shortcut_tools.clone().unwrap_or_else(|| DEFAULT_SHORTCUT_TOOLS.iter().map(|s| s.to_string()).collect())
    }

    /// Sketch S toolbar tool names (`defaults`: Onshape's ten).
    pub fn sketch_shortcut_tools(&self, defaults: &[&str]) -> Vec<String> {
        self.sketch_shortcut_tools.clone().unwrap_or_else(|| defaults.iter().map(|s| s.to_string()).collect())
    }

    pub fn toggle_sketch_shortcut(&mut self, name: &str, defaults: &[&str]) {
        let mut tools = self.sketch_shortcut_tools(defaults);
        match tools.iter().position(|t| t == name) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(name.to_string()),
        }
        self.sketch_shortcut_tools = Some(tools);
    }

    /// Drawing S toolbar tool names (`defaults`: Onshape's ten).
    pub fn drawing_shortcut_tools(&self, defaults: &[&str]) -> Vec<String> {
        self.drawing_shortcut_tools.clone().unwrap_or_else(|| defaults.iter().map(|s| s.to_string()).collect())
    }

    pub fn toggle_drawing_shortcut(&mut self, name: &str, defaults: &[&str]) {
        let mut tools = self.drawing_shortcut_tools(defaults);
        match tools.iter().position(|t| t == name) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(name.to_string()),
        }
        self.drawing_shortcut_tools = Some(tools);
    }

    pub fn toggle_shortcut(&mut self, id: &str) {
        let mut tools = self.shortcut_tools();
        match tools.iter().position(|t| t == id) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(id.to_string()),
        }
        self.shortcut_tools = Some(tools);
    }
}

// ------------------------------------------------------------ assembly toolbar

/// Onshape's assembly tool sets, in order (live dom/toolbar-assembly.txt).
pub const ASM_TOOLSETS: &[&[&str]] = &[
    &["tb-update-refs"],
    &["tb-insert"],
    &["tb-mc"],
    &["tb-fastened", "tb-revolute", "tb-slider", "tb-planar", "tb-cyl", "tb-pinslot", "tb-ball", "tb-parallel"],
    &["tb-tangent", "tb-width"],
    &["tb-group", "tb-snap", "tb-show-mates", "tb-replicate", "tb-replace", "tb-linpat", "tb-circpat", "tb-mirror"],
    &["tb-gear", "tb-rack", "tb-screw", "tb-linear"],
    &["tb-display-states", "tb-ps-in-context"],
];

/// Tools panel label and icon of a tool (live 343).
pub fn asm_tool_info(id: &str) -> (&'static str, &'static str) {
    match id {
        "tb-update-refs" => ("Update all references to latest versions", "update-all-button"),
        "tb-insert" => ("Insert", "insert"),
        "tb-mc" => ("Mate connector", "mate-connector-assembly"),
        "tb-fastened" => ("Fastened mate", "fastened"),
        "tb-revolute" => ("Revolute mate", "revolute"),
        "tb-slider" => ("Slider mate", "slider"),
        "tb-planar" => ("Planar mate", "planar"),
        "tb-cyl" => ("Cylindrical mate", "cylindrical"),
        "tb-pinslot" => ("Pin slot mate", "pin-slot"),
        "tb-ball" => ("Ball mate", "ball"),
        "tb-parallel" => ("Parallel mate", "parallel-mate"),
        "tb-tangent" => ("Tangent mate", "tangent-mate"),
        "tb-width" => ("Width mate", "width-mate"),
        "tb-group" => ("Group", "group"),
        "tb-snap" => ("Snap mode", "snap-mode"),
        "tb-show-mates" => ("Show mates mode", "show-mates"),
        "tb-replicate" => ("Replicate", "replicate"),
        "tb-replace" => ("Replace instance", "replace-part"),
        "tb-linpat" => ("Assembly linear pattern", "linear-pattern"),
        "tb-circpat" => ("Assembly circular pattern", "circular-pattern"),
        "tb-mirror" => ("Assembly mirror", "mirror"),
        "tb-gear" => ("Gear relation", "gear-relation"),
        "tb-rack" => ("Rack and pinion relation", "rack-pinion"),
        "tb-screw" => ("Screw relation", "screw-relation"),
        "tb-linear" => ("Linear relation", "linear-relation"),
        "tb-display-states" => ("Display states", "display-states"),
        _ => ("Create Part Studio in context", "create-partstudio-incontext"),
    }
}

/// The user's assembly toolbar: tool set order, removed tools, Search tools hidden.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AsmToolbarPrefs {
    #[serde(default)]
    pub order: Vec<usize>,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub hide_search: bool,
    /// S shortcut toolbar tools (toolbar tool ids; None = the defaults;
    /// Preferences › Shortcut toolbars › Assembly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut_tools: Option<Vec<String>>,
}

/// The S shortcut toolbar's default tools (live behaviours "Assembly":
/// 11 ticked of 30).
pub const ASM_DEFAULT_SHORTCUT_TOOLS: [&str; 11] = ["tb-insert", "tb-mc", "tb-fastened", "tb-revolute", "tb-slider", "tb-planar", "tb-group", "tb-replicate", "tb-replace", "tb-linpat", "tb-circpat"];

/// Tools the S toolbar offers besides the toolbar's (live: Section view,
/// Mass and section properties, Measure at the end of the list).
pub const ASM_SHORTCUT_EXTRAS: [(&str, &str, &str); 3] = [("tb-section", "Section view", "section-view-3d"), ("tb-mass", "Mass and section properties", "mass-properties"), ("tb-measure", "Measure", "measure")];

impl AsmToolbarPrefs {
    fn path() -> std::path::PathBuf {
        noshape_scene::config_dir().join("assembly_toolbar.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }

    /// Tool set order (every default set once, new sets appended).
    pub fn set_order(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self.order.iter().copied().filter(|i| *i < ASM_TOOLSETS.len()).collect();
        out.dedup();
        for i in 0..ASM_TOOLSETS.len() {
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out
    }

    pub fn shortcut_tools(&self) -> Vec<String> {
        self.shortcut_tools.clone().unwrap_or_else(|| ASM_DEFAULT_SHORTCUT_TOOLS.iter().map(|s| s.to_string()).collect())
    }

    pub fn toggle_shortcut(&mut self, id: &str) {
        let mut tools = self.shortcut_tools();
        match tools.iter().position(|t| t == id) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(id.to_string()),
        }
        self.shortcut_tools = Some(tools);
    }

    /// Tool sets as shown: (default index, tools still in the toolbar).
    pub fn sets(&self) -> Vec<(usize, Vec<&'static str>)> {
        self.set_order().into_iter().map(|i| (i, ASM_TOOLSETS[i].iter().copied().filter(|t| !self.removed.iter().any(|r| r == t)).collect::<Vec<_>>())).filter(|(_, t)| !t.is_empty()).collect()
    }
}
