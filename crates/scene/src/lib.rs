//! noshape-scene: GUI-free viewport logic shared by the app and the agent
//! API: the scene builder ([`builder`]), the sketch editor and its tools
//! ([`sketch_mode`]), the viewport [`palette`], the [`graphics_prefs`] data
//! and offscreen PNG rendering ([`png`]).

pub mod asm_scene;
pub mod builder;
pub mod camera_ops;
pub mod drawing_tools;
pub mod environment_maps;
pub mod graphics_prefs;
pub mod palette;
pub mod png;
pub mod print;
pub mod sketch_mode;
pub mod system_info;

pub use graphics_prefs::GraphicsPrefs;

/// Where documents go and what the documents page lists as "Owned by me" (created on first use): the user's
/// Documents folder's `noshape` (the OS's Known Folder, OneDrive-redirected on Windows when it is), or
/// `NOSHAPE_DOCUMENTS_DIR`. The app, the CLI and the MCP server all use this one.
pub fn documents_dir() -> std::path::PathBuf {
    let d = match std::env::var_os("NOSHAPE_DOCUMENTS_DIR") {
        Some(d) => std::path::PathBuf::from(d),
        None => dirs::document_dir().or_else(|| dirs::home_dir().map(|h| h.join("Documents"))).unwrap_or_else(|| std::path::PathBuf::from(".")).join("noshape"),
    };
    let _ = std::fs::create_dir_all(&d);
    d
}

/// `path` with a leading `~` (`~`, `~/…`, and `~\…` as Windows writes it) as the user's home folder; any other path
/// as given. Paths agents and shells write ("~/Documents/bracket.noshape") mean the home folder, which no file call
/// expands on its own.
pub fn expand_home(path: &str) -> std::borrow::Cow<'_, str> {
    match dirs::home_dir() {
        Some(home) => expand_home_in(path, &home.to_string_lossy()),
        None => std::borrow::Cow::Borrowed(path),
    }
}

/// [`expand_home`] with the home folder given.
pub fn expand_home_in<'a>(path: &'a str, home: &str) -> std::borrow::Cow<'a, str> {
    let rest = match path.strip_prefix('~') {
        Some("") => "",
        Some(r) if r.starts_with('/') || r.starts_with('\\') => r,
        _ => return std::borrow::Cow::Borrowed(path),
    };
    let home = home.trim_end_matches(['/', '\\']);
    let sep = if home.contains('\\') && !home.contains('/') { '\\' } else { '/' };
    let rest = rest.trim_start_matches(['/', '\\']);
    let rest = if sep == '\\' { rest.replace('/', "\\") } else { rest.to_string() };
    std::borrow::Cow::Owned(if rest.is_empty() { home.to_string() } else { format!("{home}{sep}{rest}") })
}

/// noshape's per-user configuration directory (created on first use;
/// `NOSHAPE_CONFIG_DIR` overrides it, for tests).
pub fn config_dir() -> std::path::PathBuf {
    let d = match std::env::var_os("NOSHAPE_CONFIG_DIR") {
        Some(d) => std::path::PathBuf::from(d),
        None => dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("noshape"),
    };
    let _ = std::fs::create_dir_all(&d);
    d
}

#[cfg(test)]
mod tests {
    use super::expand_home_in;

    /// "~/Documents/x.noshape" from an agent is the home folder's (user 11-08: "creating ~/Documents: Read-only file
    /// system"); `~\` too on Windows, in its own separators; other paths as given.
    #[test]
    fn a_leading_tilde_is_the_home_folder() {
        assert_eq!(expand_home_in("~/Documents/Paper Towel.noshape", "/Users/ann"), "/Users/ann/Documents/Paper Towel.noshape");
        assert_eq!(expand_home_in("~", "/home/ann/"), "/home/ann");
        assert_eq!(expand_home_in("~/", "/home/ann"), "/home/ann");
        assert_eq!(expand_home_in(r"~\Documents\x.noshape", r"C:\Users\ann"), r"C:\Users\ann\Documents\x.noshape");
        assert_eq!(expand_home_in("~/Documents/x.noshape", r"C:\Users\ann"), r"C:\Users\ann\Documents\x.noshape");
        for kept in ["/tmp/x.noshape", "~bob/x.noshape", "x~/y", r"C:\x.noshape", "bracket"] {
            assert_eq!(expand_home_in(kept, "/home/ann"), kept);
        }
    }
}
