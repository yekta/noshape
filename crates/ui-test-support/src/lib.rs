//! UI keystroke tests' dependencies (docs/TESTS.md "UI keystroke tests"):
//! a test crate dev-depends on this to get GPUI's test platform
//! (`#[gpui::test]`, `TestAppContext`) and `Gpu::headless`.
//!
//! It also gives the test binary its own settings and documents directories: the app reads its settings (panel widths and
//! positions, table columns, preferences, sketch options) from `NOSHAPE_CONFIG_DIR`, else the user's
//! ~/.config/noshape, where the apps run on the same machine leave theirs, and the tests' simulated clicks then
//! missed. Set before `main` (the test harness has no setup hook; nothing else runs yet), inside the run's own one if it gives one.
//! A test crate must name this crate (`use noshape_ui_test_support as _;` under `cfg(test)`) to link it.

pub mod bench;

#[used]
#[cfg_attr(any(target_os = "linux", target_os = "android", target_os = "freebsd"), unsafe(link_section = ".init_array"))]
#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,__mod_init_func"))]
#[cfg_attr(windows, unsafe(link_section = ".CRT$XCU"))]
static OWN_CONFIG_DIR: extern "C" fn() = own_config_dir;

extern "C" fn own_config_dir() {
    few_malloc_arenas();
    // (The documents page lists the documents folder: this binary's own, never the user's ~/Documents/noshape.)
    // A run's own directory (rb.sh, the tiers) holds one per binary: a binary's settings (release-ui's managed
    // workflows) never reach the next one.
    for (var, kind) in [("NOSHAPE_CONFIG_DIR", "config"), ("NOSHAPE_DOCUMENTS_DIR", "documents")] {
        let base = std::env::var_os(var).map_or_else(std::env::temp_dir, std::path::PathBuf::from);
        let dir = base.join(format!("noshape-ui-tests-{kind}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: before `main`: no other thread exists to read the environment.
        unsafe { std::env::set_var(var, &dir) };
    }
}

/// Two malloc arenas for the whole binary (glibc): each of the harness's test threads otherwise took one of its own and
/// kept what its tests freed. part-studio-ui's 16 threads held 5.4 GB a minute in, 2.4 GB with two (vibe2 11-27, at the
/// same pace), and earlyoom killed its land runs. Before `main`: glibc fixes the limit when a second thread first
/// allocates.
fn few_malloc_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn mallopt(param: i32, value: i32) -> i32;
        }
        const M_ARENA_MAX: i32 = -8;
        // SAFETY: glibc's mallopt, called before any other thread exists.
        unsafe { mallopt(M_ARENA_MAX, 2) };
    }
}

/// Whether `dir` is outside the user's ~/.config (a UI test binary's settings directory must be).
pub fn is_not_the_users(dir: &std::path::Path) -> bool {
    std::env::var_os("HOME").is_none_or(|h| !dir.starts_with(std::path::PathBuf::from(h).join(".config")))
}
