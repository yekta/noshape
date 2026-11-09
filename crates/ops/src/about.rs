//! Help › About's text: the copyright, Open CASCADE's notice and the third-party notices shipped with the app (the
//! link "Open source software components"): Open CASCADE Technology's licence (LGPL 2.1 with its exception), the
//! bundled fonts' and every Rust package the app links with its own licence files. `assets/THIRD-PARTY-NOTICES.txt`
//! is generated: `UPDATE_NOTICES=1 cargo test -p noshape-ops --test all notices::` rewrites it (tests/notices.rs),
//! and the test fails while it is out of date with Cargo.lock.

/// The copyright line (the user's, lead 11-05).
pub const COPYRIGHT: &str = "© 2026 Yekta";

/// Open CASCADE's LGPL notice, named in the dialog itself.
pub const OCCT_NOTICE: &str = "Noshape uses Open CASCADE Technology 8.0.1 (OCCT), © Open Cascade SAS, under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception (version 1.0).";

/// The notices as shipped (embedded: they open offline).
pub const NOTICES: &str = include_str!("../assets/THIRD-PARTY-NOTICES.txt");

/// The notices written where the system can open them (the app's config folder), for the About dialog's link.
pub fn notices_file() -> std::io::Result<std::path::PathBuf> {
    let path = noshape_scene::config_dir().join("THIRD-PARTY-NOTICES.txt");
    if std::fs::read_to_string(&path).ok().as_deref() == Some(NOTICES) {
        return Ok(path);
    }
    // (Written aside and renamed in: another window or test reading it never sees half a file.)
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}-{n}.tmp", std::process::id()));
    std::fs::write(&tmp, NOTICES)?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(path)
}

/// noshape's user guide, as Help › Help shows it (offline; app_user_guide).
pub const USER_GUIDE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../app/assets/help/user-guide.md"));

/// The guide's section titled `title` (a `##` or `###` heading, case aside): its heading and text, up to the next
/// heading of the same or a higher level. The app's "?" help links open these.
pub fn guide_section(title: &str) -> Option<&'static str> {
    let level = |l: &str| l.chars().take_while(|c| *c == '#').count();
    let mut at = 0;
    let mut start: Option<(usize, usize)> = None;
    for line in USER_GUIDE.split_inclusive('\n') {
        let n = level(line);
        if let Some((from, lv)) = start {
            if n > 0 && n <= lv {
                return Some(USER_GUIDE[from..at].trim_end());
            }
        } else if n >= 2 && line[n..].trim().eq_ignore_ascii_case(title.trim()) {
            start = Some((at, n));
        }
        at += line.len();
    }
    start.map(|(from, _)| USER_GUIDE[from..].trim_end())
}

/// The guide's section titles, as `guide_section` takes them.
pub fn guide_sections() -> Vec<&'static str> {
    USER_GUIDE.lines().filter(|l| l.starts_with("##")).map(|l| l.trim_start_matches('#').trim()).collect()
}
