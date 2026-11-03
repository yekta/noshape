//! This test binary's own settings directory: the app's settings (graphics, toolbars, shortcuts, recent files)
//! go to `NOSHAPE_CONFIG_DIR`, else the user's ~/.config/noshape, which two runs on one machine would share.
//! Set before `main` (the test harness has no setup hook; nothing else runs yet), unless the run already gives one.

#[used]
#[cfg_attr(any(target_os = "linux", target_os = "android", target_os = "freebsd"), unsafe(link_section = ".init_array"))]
#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,__mod_init_func"))]
#[cfg_attr(windows, unsafe(link_section = ".CRT$XCU"))]
static OWN_CONFIG_DIR: extern "C" fn() = own_config_dir;

extern "C" fn own_config_dir() {
    if std::env::var_os("NOSHAPE_CONFIG_DIR").is_some() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("noshape-ops-tests-config-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    // SAFETY: before `main`: no other thread exists to read the environment.
    unsafe { std::env::set_var("NOSHAPE_CONFIG_DIR", &dir) };
}

/// The settings directory is this process's, not the user's.
#[test]
fn settings_go_to_this_runs_own_directory() {
    let dir = noshape_scene::config_dir();
    let home = dirs_home_config();
    assert!(home.as_ref().is_none_or(|h| !dir.starts_with(h)), "{dir:?} is the user's");
}

fn dirs_home_config() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
}
