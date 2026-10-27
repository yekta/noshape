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
#[folder = "../app/assets"]
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
    // New features are named in the language ("Linear austragen 1", "Fest 1").
    noshape_model::default_names::set_localizer((code != DEFAULT).then_some(default_name as noshape_model::default_names::Localizer));
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
    #[cfg(test)]
    fn fill(&self, s: &str) -> Option<String> {
        self.fill_with(s, |_| None)
    }

    /// [`Template::fill`], a type or label value ("Hide other __label__":
    /// "planes") in the language too through `word` (live: "Andere
    /// „Ebenen“ ausblenden"); names stay as they are.
    fn fill_with(&self, s: &str, word: impl Fn(&str) -> Option<String>) -> Option<String> {
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
            let lower = name.to_lowercase();
            let typed = (lower == "label" || lower.ends_with("label") || lower.contains("type")).then(|| word(value)).flatten();
            out = out.replace(&format!("__{name}__"), typed.as_deref().unwrap_or(value));
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

static FORMATTED: OnceLock<std::sync::Mutex<HashMap<&'static str, Formatted>>> = OnceLock::new();

impl Formatted {
    fn new(code: &'static str) -> Self {
        let mut templates: HashMap<Option<char>, Vec<Template>> = HashMap::new();
        for (english, text) in table(code).iter().filter(|(k, _)| k.contains("__")) {
            if let Some(t) = Template::parse(english, text) {
                templates.entry(t.pieces[0].chars().next()).or_default().push(t);
            }
        }
        // The most specific first: "Edit __name__…" before "Edit __aliasName__".
        for list in templates.values_mut() {
            // Ties: the generic `__name__` before a specific value
            // ("Delete __name__" → „…“ löschen, not "Delete __companyType__").
            list.sort_by_key(|t| (std::cmp::Reverse(t.pieces.iter().map(|p| p.chars().count()).sum::<usize>()), !t.names.iter().all(|n| n == "name")));
        }
        Formatted { templates, seen: HashMap::new() }
    }
}

/// [`lookup`] of a formatted text, by the language's templates.
fn formatted(code: &'static str, s: &str) -> Option<&'static str> {
    let mut all = FORMATTED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let f = all.entry(code).or_insert_with(|| Formatted::new(code));
    if let Some(hit) = f.seen.get(s) {
        return *hit;
    }
    let first = s.chars().next();
    // Spacing around a text (a no-break space before a link) stays around
    // its translation.
    let trimmed = s.trim();
    let spaced = (trimmed.len() != s.len() && !trimmed.is_empty()).then(|| table(code).get(trimmed)).flatten().map(|t| s.replacen(trimmed, t, 1));
    let word = |w: &str| table(code).get(w).cloned();
    let hit = spaced.or_else(|| [first, None].iter().filter_map(|k| f.templates.get(k)).flatten().find_map(|t| t.fill_with(s, word))).or_else(|| {
        // A label with a count or a shortcut: "Mate features (2)" →
        // "Verknüpfungs-Features (2)", "Extrude (shift+e)" → "Linear
        // austragen (shift+e)" (live), when the table has the label alone.
        let (label, suffix) = s.strip_suffix(')')?.rsplit_once(" (")?;
        (!suffix.is_empty() && suffix.len() <= 16 && !suffix.contains(' ')).then_some(())?;
        table(code).get(label).map(|t| format!("{t} ({suffix})"))
    });
    // Each distinct text is kept (and its translation leaked) once; the
    // texts a session formats are few (counts, names).
    let hit: Option<&'static str> = hit.map(|t| &*Box::leak(t.into_boxed_str()));
    if f.seen.len() < 20_000 {
        f.seen.insert(s.to_string(), hit);
    }
    hit
}

/// Whether a UI text is built from a template around an entity's name
/// ("Show dependencies of __name__ …", "Create Drawing of __name__…"),
/// not a fixed label: live context menus cut those with "…" on one line
/// and wrap the fixed ones (behaviours "ui2 (10-24 08:40)"). Judged by the
/// German table, the most complete; a label or type value ("Hide all
/// __label__") doesn't count as a name.
pub fn names_an_entity(english: &str) -> bool {
    const CODE: &str = "de";
    if table(CODE).contains_key(english) || !english.contains(' ') {
        return false;
    }
    let mut all = FORMATTED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let f = all.entry(CODE).or_insert_with(|| Formatted::new(CODE));
    let named = |t: &&Template| t.names.iter().any(|n| {
        let lower = n.to_lowercase();
        !(lower == "label" || lower.ends_with("label") || lower.contains("type"))
    });
    [english.chars().next(), None].iter().filter_map(|k| f.templates.get(k)).flatten().filter(named).any(|t| t.fill_with(english, |_| None).is_some())
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

/// A text naming a stored entity ("Face of Extrude 1"): translated only by
/// a `__name__` template, keeping the name as it is ("Fläche von Extrude
/// 1"); never looked up whole, as the text may be just a name.
pub fn filled(text: &str) -> Option<String> {
    let code = current();
    if code == DEFAULT || !text.contains(' ') {
        return None;
    }
    // A selection chip "Edge of Extrude 1" (live bundle buildDisplayName):
    // "__selectionName__ of __featureNodeDisplayName__", the entity word in
    // the server strings ("Kante von Extrude 1", "Extrude 1 の 面").
    if let Some((word, name)) = text.split_once(" of ")
        && let Some(local) = default_name(word)
        && let Some(t) = table(code).get("__selectionName__ of __featureNodeDisplayName__")
    {
        return Some(t.replace("__selectionName__", local).replace("__featureNodeDisplayName__", name));
    }
    let mut all = FORMATTED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let f = all.entry(code).or_insert_with(|| Formatted::new(code));
    // Only templates that open with words ("Face of __featureName__"): one
    // starting with a value ("__a__ on __b__") could rewrite a bare name.
    let first = text.chars().next();
    let word = |w: &str| table(code).get(w).cloned();
    f.templates.get(&first)?.iter().filter(|t| t.pieces[0].chars().any(char::is_alphabetic)).find_map(|t| t.fill_with(text, word))
}

/// Onshape's template `key` filled in as it builds a text, in the UI
/// language: label values translated as words ("Hide all __label__" with
/// "instances" → "Alle „Instanzen“ ausblenden", where the table's own
/// "Hide all instances" lacks the quotes live shows). The result is drawn
/// as it is (no English key looks like it).
pub fn template(key: &str, args: &[(&str, &str)]) -> String {
    let code = current();
    let localized = code != DEFAULT;
    let mut out = if localized { table(code).get(key).cloned().unwrap_or_else(|| key.to_string()) } else { key.to_string() };
    for (name, value) in args {
        let word = localized.then(|| table(code).get(*value).cloned()).flatten();
        out = out.replace(&format!("__{name}__"), word.as_deref().unwrap_or(value));
    }
    out
}

/// A feature type's name in the UI language, as the server strings have it
/// ("Extrude" → "Linear austragen"; English otherwise).
pub fn type_name(english: &str) -> String {
    if current() == DEFAULT {
        return english.to_string();
    }
    default_name(english).unwrap_or(english).to_string()
}

/// A feature dialog's string (parameter label, enum option, group name) in
/// the UI language: as the localised feature specs have it (`specs-<code>.tsv`;
/// live keeps a spec string in English when the spec does), else as the UI
/// text. Render the result with [`crate::ui::spec_text`] (never looked up
/// again).
pub fn spec(english: &str) -> String {
    static SPECS: OnceLock<RwLock<HashMap<&'static str, &'static HashMap<String, String>>>> = OnceLock::new();
    let code = current();
    if code == DEFAULT {
        return english.to_string();
    }
    let specs = SPECS.get_or_init(Default::default);
    let cached = specs.read().unwrap_or_else(|e| e.into_inner()).get(code).copied();
    let table = cached.unwrap_or_else(|| {
        let parsed: &'static HashMap<String, String> = Box::leak(Box::new(load(&format!("specs-{code}"))));
        specs.write().unwrap_or_else(|e| e.into_inner()).insert(code, parsed);
        parsed
    });
    match table.get(english) {
        Some(t) => t.clone(),
        None => lookup(english).unwrap_or(english).to_string(),
    }
}

/// The current language's base for a new feature's default name
/// (`names-<code>.tsv`: Part Studio features as the `server` strings, mates
/// as their mate type, see `scripts/gen-i18n-tables.py`); also the entity
/// words of selection chips ("Edge"); None: English.
fn default_name(english: &str) -> Option<&'static str> {
    static NAMES: OnceLock<RwLock<HashMap<&'static str, &'static HashMap<String, String>>>> = OnceLock::new();
    let code = current();
    let names = NAMES.get_or_init(Default::default);
    let cached = names.read().unwrap_or_else(|e| e.into_inner()).get(code).copied();
    let table = cached.unwrap_or_else(|| {
        let parsed: &'static HashMap<String, String> = Box::leak(Box::new(load(&format!("names-{code}"))));
        names.write().unwrap_or_else(|e| e.into_inner()).insert(code, parsed);
        parsed
    });
    table.get(english).map(|s| s.as_str())
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
    fn menu_labels_around_names_are_told_from_fixed_ones() {
        assert!(names_an_entity("Create Drawing of skb analysis asm…"));
        assert!(names_an_entity("Show dependencies of Extrude 1…"));
        assert!(!names_an_entity("Show dependencies…"));
        assert!(!names_an_entity("Add mate connector to instance origin…"));
        assert!(!names_an_entity("Hide all instances"));
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
        // A label value is translated, a name isn't.
        let t = Template::parse("Hide other __label__", "Andere „__label__“ ausblenden").unwrap();
        assert_eq!(t.fill_with("Hide other planes", |w| (w == "planes").then(|| "Ebenen".to_string())).as_deref(), Some("Andere „Ebenen“ ausblenden"));
        let t = Template::parse("Edit __name__…", "„__name__“ bearbeiten …").unwrap();
        assert_eq!(t.fill_with("Edit planes…", |_| Some("x".into())).as_deref(), Some("„planes“ bearbeiten …"));
    }

    #[test]
    fn a_counted_label_translates_its_label() {
        assert_eq!(formatted("de", "Mate features (2)"), Some("Verknüpfungs-Features (2)"));
        assert_eq!(formatted("de", "Extrude (shift+e)"), Some("Linear austragen (shift+e)"));
        assert_eq!(formatted("de", "Mate features\u{a0}"), Some("Verknüpfungs-Features\u{a0}"));
    }

    #[test]
    fn every_language_has_its_table() {
        for (code, _) in LANGUAGES.iter().filter(|(c, _)| *c != DEFAULT) {
            assert!(load(code).len() > 5000, "{code}");
        }
        assert_eq!(load("de").get("Default geometry").map(String::as_str), Some("Standardgeometrie"));
        assert_eq!(load("fr").get("Extrude").map(String::as_str), Some("Extruder"));
    }

    #[test]
    fn entity_texts_keep_their_names() {
        let t = Template::parse("Face of __featureName__", "Fläche von __featureName__").unwrap();
        assert_eq!(t.fill("Face of Extrude 1").as_deref(), Some("Fläche von Extrude 1"));
        // Templates opening with a value never apply to entity texts.
        let t = Template::parse("__annotationType__ on __partName__", "__annotationType__ für __partName__").unwrap();
        assert!(!t.pieces[0].chars().any(char::is_alphabetic));
    }

    #[test]
    fn dialogs_take_the_feature_specs_strings() {
        // Live German Extrude: "Formschräge" (the UI text has "Entwurf"),
        // "Tiefe"; a spec string the spec leaves English stays English.
        let de = load("specs-de");
        assert_eq!(de.get("Draft").map(String::as_str), Some("Formschräge"));
        assert_eq!(de.get("Depth").map(String::as_str), Some("Tiefe"));
        assert_eq!(de.get("Move").map(String::as_str), Some("Move"));
        assert_eq!(load("de").get("Draft").map(String::as_str), Some("Entwurf"));
    }

    #[test]
    fn default_names_follow_their_sources() {
        // Live: Part Studio features as the server strings, mates as their
        // mate type (German "Verrundung 1", "Fest 1"; Japanese "固定 1").
        let de = load("names-de");
        assert_eq!(de.get("Fillet").map(String::as_str), Some("Verrundung"));
        assert_eq!(de.get("Fastened").map(String::as_str), Some("Fest"));
        assert_eq!(de.get("Mate connector").map(String::as_str), Some("Verknüpfungsverbindung"));
        assert_eq!(load("names-ja").get("Fastened").map(String::as_str), Some("固定"));
    }
}
