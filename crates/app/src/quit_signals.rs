//! SIGTERM (logout, shutdown, `kill`), SIGHUP and SIGINT quit the app as Quit does: open documents commit their
//! pending edit, then the process ends. A second signal, or 10 s without quitting, ends it at once. (Windows' logoff
//! and shutdown come as WM_ENDSESSION, macOS's as applicationWillTerminate: gpui quits on both.)

use gpui_kit::App;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;
const SIG_IGN: usize = 1;

static PIPE_WRITE: AtomicI32 = AtomicI32::new(-1);
static RECEIVED: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
    fn pipe(fds: *mut i32) -> i32;
    fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
    fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    fn _exit(code: i32) -> !;
}

extern "C" fn on_signal(sig: i32) {
    if RECEIVED.swap(true, Ordering::SeqCst) {
        unsafe { _exit(128 + sig) };
    }
    let byte = sig as u8;
    unsafe { write(PIPE_WRITE.load(Ordering::SeqCst), &byte, 1) };
}

pub fn install(cx: &mut App) {
    let mut fds = [-1i32; 2];
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        log::warn!("quit signals: no pipe ({})", std::io::Error::last_os_error());
        return;
    }
    PIPE_WRITE.store(fds[1], Ordering::SeqCst);
    let (tx, rx) = async_channel::bounded::<i32>(1);
    let read_fd = fds[0];
    let reader = std::thread::Builder::new().name("quit-signals".into()).spawn(move || {
        let mut byte = 0u8;
        loop {
            match unsafe { read(read_fd, &mut byte, 1) } {
                1 => {
                    let _ = tx.send_blocking(byte as i32);
                    // A UI thread that doesn't get to quit (hung) still ends, as by the signal's default action.
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    unsafe { _exit(128 + byte as i32) };
                }
                0 => return,
                _ if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => continue,
                _ => return,
            }
        }
    });
    if let Err(e) = reader {
        log::warn!("quit signals: no reader thread ({e})");
        return;
    }
    for sig in [SIGHUP, SIGINT, SIGTERM] {
        // Started with the signal ignored (nohup, a background job): it stays ignored.
        if unsafe { signal(sig, on_signal as extern "C" fn(i32) as usize) } == SIG_IGN {
            unsafe { signal(sig, SIG_IGN) };
        }
    }
    log::info!("quit signals: SIGTERM, SIGHUP and SIGINT quit");
    cx.spawn(async move |cx| {
        if let Ok(sig) = rx.recv().await {
            log::info!("signal {sig}: quitting");
            cx.update(|cx| cx.quit());
        }
    })
    .detach();
}
