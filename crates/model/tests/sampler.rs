//! A sampling profiler for tool tests (Linux): every running thread of the process signalled every millisecond, its
//! stack written to a file as `T <tid>: <addresses>` lines after a `BASE <load address> <exe>` line
//! (~/agents/profile.py on the perf hosts aggregates them).

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

    unsafe extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
        fn syscall(n: i64, ...) -> i64;
        fn getpid() -> i32;
        fn backtrace(buf: *mut *mut std::ffi::c_void, size: i32) -> i32;
        fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    }
    static FD: AtomicI32 = AtomicI32::new(-1);

    extern "C" fn on_signal(_: i32) {
        let mut frames = [std::ptr::null_mut(); 64];
        let n = unsafe { backtrace(frames.as_mut_ptr(), 64) }.max(0) as usize;
        let mut buf = [0u8; 64 * 17 + 16];
        let mut k = 0;
        for b in b"T 1:" {
            buf[k] = *b;
            k += 1;
        }
        for f in frames.iter().take(n) {
            buf[k] = b' ';
            k += 1;
            let a = *f as u64;
            for sh in (0..16).rev() {
                buf[k] = b"0123456789abcdef"[((a >> (sh * 4)) & 15) as usize];
                k += 1;
            }
        }
        buf[k] = b'\n';
        k += 1;
        unsafe { write(FD.load(Ordering::Relaxed), buf.as_ptr(), k) };
    }

    /// Runs `f`, the process sampled into `out`.
    pub fn sample<R>(out: &std::path::Path, f: impl FnOnce() -> R) -> R {
        use std::io::Write;
        use std::os::fd::IntoRawFd;
        let mut file = std::fs::File::create(out).unwrap();
        let exe = std::fs::read_link("/proc/self/exe").unwrap_or_default();
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap_or_default();
        if let Some(l) = maps.lines().find(|l| l.ends_with(&*exe.to_string_lossy())) {
            writeln!(file, "BASE {} {}", l.split('-').next().unwrap_or("0"), exe.display()).unwrap();
        }
        FD.store(file.into_raw_fd(), Ordering::Relaxed);
        unsafe {
            let mut warm = [std::ptr::null_mut(); 4];
            backtrace(warm.as_mut_ptr(), 4);
            signal(12, on_signal as *const () as usize);
        }
        let pid = unsafe { getpid() } as i64;
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let h = std::thread::spawn(move || {
            let me = unsafe { syscall(186) };
            while !s2.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(1));
                let Ok(dir) = std::fs::read_dir("/proc/self/task") else { continue };
                for e in dir.flatten() {
                    let Some(tid) = e.file_name().to_str().and_then(|s| s.parse::<i64>().ok()) else { continue };
                    if tid == me {
                        continue;
                    }
                    // Running threads only (a CPU profile).
                    let stat = std::fs::read_to_string(e.path().join("stat")).unwrap_or_default();
                    if stat.rsplit(')').next().is_some_and(|r| r.trim_start().starts_with('R')) {
                        unsafe { syscall(234, pid, tid, 12i64) };
                    }
                }
            }
        });
        let r = f();
        stop.store(true, Ordering::Relaxed);
        h.join().unwrap();
        r
    }
}

#[cfg(target_os = "linux")]
pub use imp::sample;

/// (Elsewhere: runs `f`, unsampled.)
#[cfg(not(target_os = "linux"))]
pub fn sample<R>(_out: &std::path::Path, f: impl FnOnce() -> R) -> R {
    f()
}
