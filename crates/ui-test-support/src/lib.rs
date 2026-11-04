//! UI keystroke tests' dependencies (docs/TESTS.md "UI keystroke tests"):
//! a test crate dev-depends on this to get GPUI's test platform
//! (`#[gpui::test]`, `TestAppContext`) and `Gpu::headless`.
//!
//! It also gives the test binary its own settings directory: the app reads its settings (panel widths and
//! positions, table columns, preferences, sketch options) from `NOSHAPE_CONFIG_DIR`, else the user's
//! ~/.config/noshape, where the apps run on the same machine leave theirs, and the tests' simulated clicks then
//! missed. Set before `main` (the test harness has no setup hook; nothing else runs yet) unless the run gives one.
//! A test crate must name this crate (`use noshape_ui_test_support as _;` under `cfg(test)`) to link it.

#[used]
#[cfg_attr(any(target_os = "linux", target_os = "android", target_os = "freebsd"), unsafe(link_section = ".init_array"))]
#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,__mod_init_func"))]
#[cfg_attr(windows, unsafe(link_section = ".CRT$XCU"))]
static OWN_CONFIG_DIR: extern "C" fn() = own_config_dir;

extern "C" fn own_config_dir() {
    if std::env::var_os("NOSHAPE_CONFIG_DIR").is_some() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("noshape-ui-tests-config-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    // SAFETY: before `main`: no other thread exists to read the environment.
    unsafe { std::env::set_var("NOSHAPE_CONFIG_DIR", &dir) };
}

/// Whether `dir` is outside the user's ~/.config (a UI test binary's settings directory must be).
pub fn is_not_the_users(dir: &std::path::Path) -> bool {
    std::env::var_os("HOME").is_none_or(|h| !dir.starts_with(std::path::PathBuf::from(h).join(".config")))
}
