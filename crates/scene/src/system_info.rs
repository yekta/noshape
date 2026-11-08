//! The machine for Help › System check: memory and disk (std says neither), and whether a GPU adapter is a software
//! renderer.

use std::path::Path;

/// Physical memory in bytes: total, and available where the OS says it cheaply.
pub fn memory() -> Option<(u64, Option<u64>)> {
    memory_imp()
}

/// The disk holding `path`, in bytes: free to this user, and its size.
pub fn disk(path: &Path) -> Option<(u64, u64)> {
    disk_imp(path)
}

/// A software renderer's adapter ("llvmpipe (Vulkan)"): Onshape flags software GL renderers (SwiftShader) as a
/// performance error; ours are llvmpipe, lavapipe, WARP and the like.
pub fn is_software_adapter(adapter: &str) -> bool {
    let a = adapter.to_lowercase();
    ["llvmpipe", "lavapipe", "softpipe", "swiftshader", "software", "basic render"].iter().any(|k| a.contains(k))
}

#[cfg(target_os = "linux")]
fn memory_imp() -> Option<(u64, Option<u64>)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)?.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()).map(|k| k * 1024);
    Some((kb("MemTotal:")?, kb("MemAvailable:")))
}

#[cfg(target_os = "macos")]
fn memory_imp() -> Option<(u64, Option<u64>)> {
    let mut total: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: "hw.memsize" is a u64 sysctl; `total` and `len` describe that buffer.
    let r = unsafe { libc::sysctlbyname(c"hw.memsize".as_ptr(), (&mut total as *mut u64).cast(), &mut len, std::ptr::null_mut(), 0) };
    (r == 0).then_some((total, None))
}

#[cfg(windows)]
fn memory_imp() -> Option<(u64, Option<u64>)> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: `m` is a MEMORYSTATUSEX with its length set, as the call requires.
    unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
    Some((m.ullTotalPhys, Some(m.ullAvailPhys)))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn memory_imp() -> Option<(u64, Option<u64>)> {
    None
}

#[cfg(unix)]
fn disk_imp(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: a zeroed statvfs is a valid out-buffer; `c` is a NUL-terminated path.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let block = s.f_frsize as u64;
    Some((s.f_bavail as u64 * block, s.f_blocks as u64 * block))
}

#[cfg(windows)]
fn disk_imp(path: &Path) -> Option<(u64, u64)> {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let dir = windows::core::HSTRING::from(path);
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: the out-pointers are live u64s for the call's duration.
    unsafe { GetDiskFreeSpaceExW(&dir, Some(&mut free), Some(&mut total), None) }.ok()?;
    Some((free, total))
}

#[cfg(not(any(unix, windows)))]
fn disk_imp(_: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_machine_has_memory_and_a_disk() {
        let (total, available) = memory().expect("memory");
        assert!(total > 0 && available.is_none_or(|a| a <= total));
        let (free, size) = disk(&std::env::temp_dir()).expect("disk");
        assert!(size > 0 && free <= size);
    }
}
