//! A standard content configuration's Part number and Description (the Insert panel's pencil, an instance's
//! Properties…; live skb 10-20): kept company / user wide in the config directory, keyed by the full configuration,
//! not in the document.

use noshape_model::assembly::standard::{self, StandardSpec};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StdProps {
    #[serde(default)]
    pub part_number: String,
    /// None = the generated description.
    #[serde(default)]
    pub description: Option<String>,
}

fn path() -> std::path::PathBuf {
    noshape_scene::config_dir().join("standard_content_properties.json")
}

/// The configuration's key: the spec with its defaults spelt out.
fn key(spec: &StandardSpec) -> String {
    let mut s = spec.clone();
    s.material = Some(spec.material().to_string());
    s.finish = Some(spec.finish().to_string());
    if let Some(c) = standard::component(spec) {
        for p in &c.params {
            s.params.insert(p.label.clone(), spec.param(&p.label).to_string());
        }
    }
    serde_json::to_string(&s).unwrap_or_default()
}

fn load() -> BTreeMap<String, StdProps> {
    let saved: BTreeMap<String, StdProps> = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Keys saved before the generic parameters (head_slot, series…) read again as specs and keyed the current way.
    saved.into_iter().map(|(k, v)| (serde_json::from_str::<StandardSpec>(&k).map(|s| key(&s)).unwrap_or(k), v)).collect()
}

/// The configuration's saved properties (empty: none saved).
pub fn std_props(spec: &StandardSpec) -> StdProps {
    load().remove(&key(spec)).unwrap_or_default()
}

/// Save the configuration's properties (both empty: forgotten).
pub fn save_std_props(spec: &StandardSpec, props: StdProps) {
    let mut all = load();
    let k = key(spec);
    if props.part_number.is_empty() && props.description.is_none() {
        all.remove(&k);
    } else {
        all.insert(k, props);
    }
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(t) = serde_json::to_string_pretty(&all) {
        let _ = std::fs::write(p, t);
    }
}
