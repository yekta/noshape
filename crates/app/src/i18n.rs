//! UI language (My account › Preferences › Language) and the string table.
//!
//! Strings are keyed by their English text, as Onshape's bundle does: `tr("Save
//! language")` returns the current language's text, or the English key when
//! the table has none. Tables are `assets/i18n/<code>.tsv` (`English\tTranslated`
//! per line, `\n` / `\t` escaped), loaded on first use of a language.
//! The language is stored with the preferences; changing it re-renders every
//! window (live: the page reloads in the new language).
//!
//! Plain UI text is translated where it's drawn: the patched gpui looks every
//! `&str` / `String` / `SharedString` text child up in the current table
//! (`gpui_kit::set_text_translator`). User-authored text (document, tab,
//! part, feature… names, notes, properties) must render through
//! [`crate::ui::user_text`], which the lookup skips: a part named "Extrude"
//! stays "Extrude" in every language.
//!
//! QA: `NOSHAPE_PSEUDO_LOCALE=1` translates every plain text to `⟦text⟧`,
//! and `NOSHAPE_I18N_LOG=<file>` logs each painted text, where, and whether
//! it was looked up (the guard `scripts/qa-i18n-user-text.sh` checks that
//! user content never is).

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
    install_translator(code);
}

fn install_translator(code: &str) {
    if std::env::var_os("NOSHAPE_PSEUDO_LOCALE").is_some() {
        gpui_kit::set_text_translator(Some(pseudo));
    } else if code == DEFAULT {
        gpui_kit::set_text_translator(None);
    } else {
        gpui_kit::set_text_translator(Some(lookup));
    }
    // New features are named in the language ("Linear austragen 1").
    noshape_model::default_names::set_localizer((code != DEFAULT).then_some(lookup as noshape_model::default_names::Localizer));
    if let Some(path) = std::env::var_os("NOSHAPE_I18N_LOG") {
        let _ = LOG.set(std::sync::Mutex::new((std::fs::File::create(path).ok(), Default::default())));
        gpui_kit::set_translated_text_log(Some(log_text));
    }
}

/// The current table's text for a UI string: the string itself, else a
/// formatted text matching one of its `__name__` templates ("Parts (3)" by
/// "Parts (__numberOfParts__)" → "Bauteile (3)").
fn lookup(english: &str) -> Option<&'static str> {
    let code = current();
    if let Some(t) = table(code).get(english) {
        return Some(t.as_str());
    }
    if !english.chars().any(char::is_alphabetic) {
        return None;
    }
    formatted(code, english)
}

/// A table entry with `__name__` placeholders: its literal pieces around
/// them, their names, and the translation.
struct Template {
    pieces: Vec<String>,
    names: Vec<String>,
    text: String,
}

impl Template {
    fn parse(english: &str, text: &str) -> Option<Self> {
        let mut pieces = Vec::new();
        let mut names = Vec::new();
        let mut rest = english;
        while let Some(at) = rest.find("__") {
            let end = rest[at + 2..].find("__")? + at + 2;
            let name = &rest[at + 2..end];
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            pieces.push(rest[..at].to_string());
            names.push(name.to_string());
            rest = &rest[end + 2..];
        }
        pieces.push(rest.to_string());
        // Only templates with words around the values ("__a__ __b__" would
        // match any text).
        (!names.is_empty() && pieces.iter().any(|p| p.chars().any(char::is_alphabetic))).then(|| Self { pieces, names, text: text.to_string() })
    }

    /// The translation of `s` if it's this template filled in (values
    /// non-empty, each up to the next piece's first occurrence).
    fn fill(&self, s: &str) -> Option<String> {
        let mut rest = s.strip_prefix(self.pieces[0].as_str())?;
        let mut values = Vec::with_capacity(self.names.len());
        for (i, piece) in self.pieces[1..].iter().enumerate() {
            let last = i + 2 == self.pieces.len();
            let at = if last {
                rest.len().checked_sub(piece.len()).filter(|&at| rest.is_char_boundary(at) && rest[at..] == **piece)?
            } else if piece.is_empty() {
                return None;
            } else {
                rest.char_indices().skip(1).find(|(at, _)| rest[*at..].starts_with(piece.as_str()))?.0
            };
            if at == 0 {
                return None;
            }
            values.push(&rest[..at]);
            rest = &rest[at + piece.len()..];
        }
        let mut out = self.text.clone();
        for (name, value) in self.names.iter().zip(values) {
            out = out.replace(&format!("__{name}__"), value);
        }
        Some(out)
    }
}

/// The templates of a language, by their first character (None: they start
/// with a value), and the formatted texts looked up so far.
struct Formatted {
    templates: HashMap<Option<char>, Vec<Template>>,
    seen: HashMap<String, Option<&'static str>>,
}

/// [`lookup`] of a formatted text, by the language's templates.
fn formatted(code: &'static str, s: &str) -> Option<&'static str> {
    static FORMATTED: OnceLock<std::sync::Mutex<HashMap<&'static str, Formatted>>> = OnceLock::new();
    let mut all = FORMATTED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let f = all.entry(code).or_insert_with(|| {
        let mut templates: HashMap<Option<char>, Vec<Template>> = HashMap::new();
        for (english, text) in table(code).iter().filter(|(k, _)| k.contains("__")) {
            if let Some(t) = Template::parse(english, text) {
                templates.entry(t.pieces[0].chars().next()).or_default().push(t);
            }
        }
        Formatted { templates, seen: HashMap::new() }
    });
    if let Some(hit) = f.seen.get(s) {
        return *hit;
    }
    let first = s.chars().next();
    let hit = [first, None].iter().filter_map(|k| f.templates.get(k)).flatten().find_map(|t| t.fill(s));
    // Each distinct text is kept (and its translation leaked) once; the
    // texts a session formats are few (counts, names).
    let hit: Option<&'static str> = hit.map(|t| &*Box::leak(t.into_boxed_str()));
    if f.seen.len() < 20_000 {
        f.seen.insert(s.to_string(), hit);
    }
    hit
}

/// QA pseudo-locale: every plain text shows as `⟦text⟧` (interned).
fn pseudo(english: &str) -> Option<&'static str> {
    static SEEN: OnceLock<std::sync::Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    if english.trim().is_empty() || english.starts_with('⟦') {
        return None;
    }
    let mut seen = SEEN.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    Some(*seen.entry(english.to_string()).or_insert_with(|| Box::leak(format!("⟦{english}⟧").into_boxed_str())))
}

/// The `NOSHAPE_I18N_LOG` file and the lines written so far (each text is
/// painted every frame; it's logged once per place).
type TextLog = (Option<std::fs::File>, std::collections::HashSet<String>);
static LOG: OnceLock<std::sync::Mutex<TextLog>> = OnceLock::new();

/// One painted text for the QA log: `x  y  w  h  T|U  text`, `T` for plain
/// text (looked up in the table), `U` for user text (never).
fn log_text(text: &str, b: gpui_kit::Bounds<gpui_kit::Pixels>, looked_up: bool) {
    use std::io::Write as _;
    if let Some(Ok(mut log)) = LOG.get().map(|m| m.lock()) {
        let line = format!("{}\t{}\t{}\t{}\t{}\t{}", f32::from(b.origin.x).round(), f32::from(b.origin.y).round(), f32::from(b.size.width).round(), f32::from(b.size.height).round(), if looked_up { 'T' } else { 'U' }, text.replace('\n', " "));
        let (file, seen) = &mut *log;
        if let Some(f) = file.as_mut()
            && seen.insert(line.clone())
        {
            let _ = writeln!(f, "{line}");
        }
    }
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
    let unescape = |s: &str| {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        }
        out
    };
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

    #[test]
    fn formatted_texts_match_templates() {
        let t = Template::parse("Parts (__numberOfParts__)", "Bauteile (__numberOfParts__)").unwrap();
        assert_eq!(t.fill("Parts (12)").as_deref(), Some("Bauteile (12)"));
        assert_eq!(t.fill("Parts ()"), None);
        assert_eq!(t.fill("Parts 12"), None);
        let t = Template::parse("Features (__shownCount__ of __featureCount__)", "Features (__shownCount__ von __featureCount__)").unwrap();
        assert_eq!(t.fill("Features (3 of 8)").as_deref(), Some("Features (3 von 8)"));
        assert!(Template::parse("__a__ __b__", "x").is_none(), "no words: would match anything");
        assert_eq!(Template::parse("Edit __name__", "Modifier __name__").unwrap().fill("Edit Extrude 1").as_deref(), Some("Modifier Extrude 1"));
    }

    #[test]
    fn every_language_has_its_table() {
        for (code, _) in LANGUAGES.iter().filter(|(c, _)| *c != DEFAULT) {
            assert!(load(code).len() > 5000, "{code}");
        }
        assert_eq!(load("de").get("Default geometry").map(String::as_str), Some("Standardgeometrie"));
        assert_eq!(load("fr").get("Extrude").map(String::as_str), Some("Extruder"));
    }
}
