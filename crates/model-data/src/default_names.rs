//! The language of new features' default names. Onshape names a new feature
//! in the UI language and stores that name ("Linear austragen 1" in German,
//! "固定 1" for a fastened mate in Japanese); stored names are never
//! translated afterwards. The app installs its string table; without one
//! (scripts, the agent API) names are English.

use std::sync::RwLock;

/// The lookup of an English UI string in the current language.
pub type Localizer = fn(&str) -> Option<&'static str>;

static LOCALIZER: RwLock<Option<Localizer>> = RwLock::new(None);

/// Install (or remove) the lookup of a default name's base ("Extrude").
pub fn set_localizer(f: Option<Localizer>) {
    *LOCALIZER.write().unwrap_or_else(|e| e.into_inner()) = f;
}

/// `english` ("Extrude") in the current language.
pub fn base(english: &str) -> String {
    base_with(*LOCALIZER.read().unwrap_or_else(|e| e.into_inner()), english)
}

fn base_with(f: Option<Localizer>, english: &str) -> String {
    f.and_then(|f| f(english)).unwrap_or(english).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_names_follow_the_localizer() {
        fn de(s: &str) -> Option<&'static str> {
            (s == "Extrude").then_some("Linear austragen")
        }
        assert_eq!(super::base_with(Some(de), "Extrude"), "Linear austragen");
        assert_eq!(super::base_with(Some(de), "Fillet"), "Fillet", "no translation: English");
        assert_eq!(super::base_with(None, "Extrude"), "Extrude");
    }
}
