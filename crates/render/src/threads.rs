//! Background work's thread priority: a regeneration and its meshing take every core, and the UI thread and the
//! frames waited for one (hat8's open: the UI thread late by 0.7–2 s while it used 8–23 ms of CPU).

use std::cell::Cell;

/// This thread a little below the UI thread and the frames (once per thread; the threads it starts inherit it):
/// Linux nice 5, macOS user-initiated (not utility: that one may run on efficiency cores), Windows below normal.
pub fn lower_priority() {
    thread_local! { static DONE: Cell<bool> = const { Cell::new(false) }; }
    if DONE.with(|d| d.replace(true)) {
        return;
    }
    #[cfg(target_os = "linux")]
    // SAFETY: plain syscalls on this thread's own id.
    unsafe {
        // (Never back up: a process started niced keeps its niceness.)
        let tid = libc::gettid() as libc::id_t;
        if libc::getpriority(libc::PRIO_PROCESS, tid) < 5 {
            libc::setpriority(libc::PRIO_PROCESS, tid, 5);
        }
    }
    #[cfg(target_os = "macos")]
    // SAFETY: sets this thread's own QoS class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
    }
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetCurrentThread() -> isize;
            fn SetThreadPriority(thread: isize, priority: i32) -> i32;
        }
        // THREAD_PRIORITY_BELOW_NORMAL.
        // SAFETY: the pseudo handle of this thread.
        unsafe {
            SetThreadPriority(GetCurrentThread(), -1);
        }
    }
}

#[cfg(test)]
mod tests {
    /// A thread doing background work runs at nice 5 at least; the UI thread (the caller of nothing here) doesn't change.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_background_thread_runs_below_the_ui_thread() {
        // SAFETY: reading thread priorities.
        let nice = || unsafe { libc::getpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t) };
        let here = nice();
        let there = std::thread::spawn(move || {
            super::lower_priority();
            super::lower_priority();
            nice()
        })
        .join()
        .unwrap();
        assert!(there >= 5.max(here), "background thread at nice {there}, this one {here}");
        assert_eq!(nice(), here, "the calling thread keeps its priority");
    }
}
