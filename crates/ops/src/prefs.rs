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
/// (crates/app/src/graphics_prefs_dialog.rs rows).
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
