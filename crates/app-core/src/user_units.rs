//! The user's units and formats (Preferences › Decimal format and Units; moved from the app's `units_prefs`, which
//! has the page): the default units of new documents, the time format and the decimal separator, kept in the config
//! dir. New documents (`new_document`) start with these units; document times follow the time format
//! (`start_library::format_onshape_time`).

use noshape_units::UnitPrefs;
use serde::{Deserialize, Serialize};

/// Display decimals options ("0", "0.1", … "0.12345678").
pub const DECIMALS: [&str; 9] = ["0", "0.1", "0.12", "0.123", "0.1234", "0.12345", "0.123456", "0.1234567", "0.12345678"];
/// How many [`DECIMALS`] options each of `OTHER_UNITS` offers (live).
pub const OTHER_DECIMALS: [usize; 9] = [7, 7, 8, 9, 9, 9, 8, 9, 9];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UserUnits {
    pub units: UnitPrefs,
    /// Time format "24 hour" (default "12 hour").
    pub time_24h: bool,
    /// Decimal format › Use comma as decimal separator.
    pub comma_decimal: bool,
}

fn path() -> std::path::PathBuf {
    crate::paths::config_dir().join("units.json")
}

static CACHE: parking_lot::RwLock<Option<UserUnits>> = parking_lot::RwLock::new(None);

/// The saved preferences (read once, then cached).
pub fn current() -> UserUnits {
    if let Some(u) = *CACHE.read() {
        return u;
    }
    let u = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    *CACHE.write() = Some(u);
    u
}

pub fn save(u: UserUnits) {
    *CACHE.write() = Some(u);
    if let Ok(t) = serde_json::to_string_pretty(&u) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, t);
    }
}

/// Applies the saved decimal separator (at startup).
pub fn apply() {
    noshape_units::set_comma_decimal(current().comma_decimal);
}

/// A new, empty document with the user's default units.
pub fn new_document() -> noshape_model::Document {
    let mut d = noshape_model::Document::new();
    d.settings.units = current().units;
    d
}

/// Document times use the 24 hour clock.
pub fn time_24h() -> bool {
    current().time_24h
}
