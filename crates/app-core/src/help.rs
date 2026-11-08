//! The app's "?" help links and "Learn more" links: the offline user guide at a section (no links to Onshape's
//! website). The app sets the opener at startup; the crates below it call [`open`].

use gpui_kit::App;
use std::sync::OnceLock;

static OPENER: OnceLock<fn(&str, &mut App)> = OnceLock::new();

/// The app's guide window (`noshape::help_menu::open_section`).
pub fn set_opener(f: fn(&str, &mut App)) {
    let _ = OPENER.set(f);
}

/// The user guide's section titled `section` (`noshape_ops::about::guide_section`).
pub fn open(section: &str, cx: &mut App) {
    if let Some(f) = OPENER.get() {
        f(section, cx);
    }
}
