//! The user's preference files (per user, JSON in noshape's config
//! directory), shared by the app and the agent API: Graphics preferences.

use noshape_scene::GraphicsPrefs;
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
