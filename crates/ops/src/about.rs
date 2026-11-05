//! Help › About's text: the copyright, Open CASCADE's notice and the third-party notices shipped with the app (the
//! link "Open source software components"): Open CASCADE Technology's licence (LGPL 2.1 with its exception), the
//! bundled fonts' and every Rust package the app links with its own licence files. `assets/THIRD-PARTY-NOTICES.txt`
//! is generated: `UPDATE_NOTICES=1 cargo test -p noshape-ops --test all notices::` rewrites it (tests/notices.rs),
//! and the test fails while it is out of date with Cargo.lock.

/// The copyright line (the user's, lead 11-05).
pub const COPYRIGHT: &str = "© 2026 Yekta";

/// Open CASCADE's LGPL notice, named in the dialog itself.
pub const OCCT_NOTICE: &str = "noshape uses Open CASCADE Technology 8.0.1 (OCCT), © Open Cascade SAS, under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception (version 1.0).";

/// The notices as shipped (embedded: they open offline).
pub const NOTICES: &str = include_str!("../assets/THIRD-PARTY-NOTICES.txt");

/// The notices written where the system can open them (the app's config folder), for the About dialog's link.
pub fn notices_file() -> std::io::Result<std::path::PathBuf> {
    let path = noshape_scene::config_dir().join("THIRD-PARTY-NOTICES.txt");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(NOTICES) {
        std::fs::write(&path, NOTICES)?;
    }
    Ok(path)
}
