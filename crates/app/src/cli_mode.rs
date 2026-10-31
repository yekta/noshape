//! The app binary is the CLI too: `noshape mcp` and `noshape <command> …` run
//! the agent API (noshape-cli's library) without starting a window.

use std::ffi::OsString;

/// Run the CLI when the first argument is one of its commands; else `None`
/// (the app opens). Returns the exit code.
pub fn run(args: &[OsString]) -> Option<i32> {
    if !args.get(1).is_some_and(|a| noshape_cli::is_command(a)) {
        return None;
    }
    #[cfg(windows)]
    console::attach();
    Some(noshape_cli::run(args.to_vec()))
}

/// macOS: `noshape <file>` through the command line tool's link (outside the
/// .app) opens the app bundle instead, so it runs as a regular app (its
/// bundle, icon and shaders), the way `open` starts it. True when it did.
pub fn reopen_as_bundle(args: &[OsString]) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let Ok(real) = exe.canonicalize() else { return false };
    let in_bundle = |p: &std::path::Path| p.parent().and_then(|d| d.parent()).is_some_and(|c| c.ends_with("Contents")) && p.to_string_lossy().contains(".app/");
    if in_bundle(&exe) || !in_bundle(&real) {
        return false;
    }
    let Some(bundle) = real.ancestors().nth(3) else { return false };
    let cwd = std::env::current_dir().unwrap_or_default();
    // Paths relative to the terminal's folder, which the app doesn't start in.
    let rest = args.iter().skip(1).map(|a| if a.to_string_lossy().starts_with('-') { a.clone() } else { cwd.join(a).into_os_string() });
    std::process::Command::new("/usr/bin/open").arg("-n").arg("-a").arg(bundle).arg("--args").args(rest).status().is_ok_and(|s| s.success())
}

/// Windows: the release app is a GUI-subsystem program, so it has no console
/// of its own. The standard handles a caller passes (an MCP client's pipes, a
/// redirect to a file) are used as they are; the missing ones write to the
/// console of the terminal it was started from.
#[cfg(windows)]
mod console {
    use std::ffi::c_void;

    type Handle = *mut c_void;
    const STD_HANDLES: [u32; 3] = [-10i32 as u32, -11i32 as u32, -12i32 as u32];
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ_WRITE: u32 = 3;
    const OPEN_EXISTING: u32 = 3;
    const FILE_TYPE_UNKNOWN: u32 = 0;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
        fn GetStdHandle(id: u32) -> Handle;
        fn SetStdHandle(id: u32, handle: Handle) -> i32;
        fn GetFileType(handle: Handle) -> u32;
        fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut c_void, disposition: u32, flags: u32, template: Handle) -> Handle;
    }

    fn usable(h: Handle) -> bool {
        !h.is_null() && h as isize != -1 && unsafe { GetFileType(h) } != FILE_TYPE_UNKNOWN
    }

    pub fn attach() {
        let missing: Vec<u32> = STD_HANDLES.into_iter().filter(|id| !usable(unsafe { GetStdHandle(*id) })).collect();
        if missing.is_empty() || unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
            return;
        }
        for id in missing {
            let name: Vec<u16> = if id == STD_HANDLES[0] { "CONIN$" } else { "CONOUT$" }.encode_utf16().chain([0]).collect();
            let h = unsafe { CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
            if usable(h) {
                unsafe { SetStdHandle(id, h) };
            }
        }
    }
}
