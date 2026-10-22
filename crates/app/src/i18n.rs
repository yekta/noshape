//! UI language (My account › Preferences › Language) and the string table.
//!
//! Strings are keyed by their English text, as Onshape's bundle does: `tr("Save
//! language")` returns the current language's text, or the English key when
//! the table has none. Tables are `assets/i18n/<code>.tsv` (`English\tTranslated`
//! per line, `\n` / `\t` escaped), loaded on first use of a language.
//! The language is stored with the preferences; changing it re-renders every
//! window (live: the page reloads in the new language).

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Onshape's languages, in its select's order (live prefs, behaviours "My
/// account › Preferences"): (code, name as listed).
pub const LANGUAGES: [(&str, &str); 9] = [
    ("zh-TW", "繁體中文"),
    ("fr", "Français"),
    ("en", "English"),
    ("de", "Deutsch"),
    ("zh-CN", "简体中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("es", "Español"),
    ("it", "Italiano"),
];

/// The default language.
pub const DEFAULT: &str = "en";

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "i18n/*.tsv"]
struct Tables;

static CURRENT: RwLock<&'static str> = RwLock::new(DEFAULT);
static TABLES: OnceLock<RwLock<HashMap<&'static str, &'static HashMap<String, String>>>> = OnceLock::new();

/// The current language code.
pub fn current() -> &'static str {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

/// Switch the UI language (a code of [`LANGUAGES`]; unknown codes mean English).
pub fn set_current(code: &str) {
    let code = LANGUAGES.iter().map(|(c, _)| *c).find(|c| *c == code).unwrap_or(DEFAULT);
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = code;
}

fn table(code: &'static str) -> &'static HashMap<String, String> {
    let tables = TABLES.get_or_init(Default::default);
    if let Some(t) = tables.read().unwrap_or_else(|e| e.into_inner()).get(code) {
        return t;
    }
    let parsed: &'static HashMap<String, String> = Box::leak(Box::new(load(code)));
    tables.write().unwrap_or_else(|e| e.into_inner()).insert(code, parsed);
    parsed
}

fn load(code: &str) -> HashMap<String, String> {
    let unescape = |s: &str| s.replace("\\n", "\n").replace("\\t", "\t");
    Tables::get(&format!("i18n/{code}.tsv"))
        .and_then(|f| String::from_utf8(f.data.into_owned()).ok())
        .map(|text| text.lines().filter_map(|l| l.split_once('\t')).map(|(k, v)| (unescape(k), unescape(v))).collect())
        .unwrap_or_default()
}

/// The UI text for `english` in the current language.
pub fn tr(english: &str) -> String {
    let code = current();
    if code == DEFAULT {
        return english.to_string();
    }
    table(code).get(english).cloned().unwrap_or_else(|| english.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_the_key_and_the_fallback() {
        set_current("en");
        assert_eq!(tr("Save language"), "Save language");
        set_current("xx");
        assert_eq!(current(), "en");
        assert_eq!(LANGUAGES.iter().position(|(c, _)| *c == DEFAULT), Some(2));
    }
}
