//! noshape.com (Windows), installed beside noshape.exe: what a terminal runs for `noshape` (cmd and PowerShell try
//! .com before .exe). noshape.exe is a GUI program, so a terminal doesn't wait for it and its output lands after the
//! next prompt; this console program runs it with the terminal's stdin, stdout and stderr and waits, so `noshape
//! <command>` prints before the prompt returns and exits with the command's code. Opening the app (no command, a
//! document, a --demo flag) doesn't keep the terminal waiting.

use std::ffi::OsString;
use std::process::{Command, Stdio, exit};

/// Whether the first argument opens the app rather than runs a command. The app decides with the operation
/// registry (noshape-cli `is_command`); commands and groups are lowercase words, so a path or a flag other than
/// --help / --version opens the app.
fn opens_app(first: Option<&str>) -> bool {
    match first {
        None => true,
        Some("--help" | "-h" | "--version" | "-V") => false,
        Some(a) => !a.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
    }
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let app = match std::env::current_exe() {
        Ok(me) => me.with_extension("exe"),
        Err(e) => {
            eprintln!("noshape: {e}");
            exit(1);
        }
    };
    if !app.is_file() {
        eprintln!("noshape: {} not found", app.display());
        exit(1);
    }
    let mut cmd = Command::new(&app);
    cmd.args(&args);
    if opens_app(args.first().and_then(|a| a.to_str())) {
        // The window on its own: no console, no wait, and none of this process's handles (a pipe it holds would keep
        // whoever reads this command's output waiting until the window closes).
        keep_std_handles_to_self();
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        match cmd.spawn() {
            Ok(_) => exit(0),
            Err(e) => {
                eprintln!("noshape: {}: {e}", app.display());
                exit(1);
            }
        }
    }
    // Ctrl+C reaches noshape.exe too (it joins this console); this one waits for it to end.
    ignore_ctrl_c();
    match cmd.status() {
        Ok(s) => exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("noshape: {}: {e}", app.display());
            exit(1);
        }
    }
}

#[cfg(windows)]
fn ignore_ctrl_c() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: *const std::ffi::c_void, add: i32) -> i32;
    }
    unsafe { SetConsoleCtrlHandler(std::ptr::null(), 1) };
}

#[cfg(not(windows))]
fn ignore_ctrl_c() {}

/// This process's standard handles stay out of the processes it starts (Windows passes every inheritable handle on).
#[cfg(windows)]
fn keep_std_handles_to_self() {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(id: u32) -> *mut c_void;
        fn SetHandleInformation(handle: *mut c_void, mask: u32, flags: u32) -> i32;
    }
    const HANDLE_FLAG_INHERIT: u32 = 1;
    for id in [-10i32 as u32, -11i32 as u32, -12i32 as u32] {
        let h = unsafe { GetStdHandle(id) };
        if !h.is_null() && h as isize != -1 {
            unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0) };
        }
    }
}

#[cfg(not(windows))]
fn keep_std_handles_to_self() {}

#[cfg(test)]
mod tests {
    #[test]
    fn commands_wait_and_the_app_does_not() {
        for a in ["mcp", "tools", "document", "agents", "--version", "-V", "--help"] {
            assert!(!super::opens_app(Some(a)), "{a}");
        }
        for a in [None, Some("--demo"), Some(r"C:\docs\plate.noshape"), Some("plate.noshape"), Some("Plate")] {
            assert!(super::opens_app(a), "{a:?}");
        }
    }
}
