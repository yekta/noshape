//! Windows backend of the selection-count cursor (see cursor.rs): an icon
//! cursor from the image, and a window subclass that answers WM_SETCURSOR
//! with it while it is active (GPUI sets its own cursor on every
//! WM_SETCURSOR).

use super::CursorImage;
use raw_window_handle::RawWindowHandle;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HCURSOR, HTCLIENT, ICONINFO, IDC_ARROW, LoadCursorW, SetCursor, WM_SETCURSOR};

/// The active count cursor (an HCURSOR), 0 when GPUI's cursor applies.
static ACTIVE: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static CURSORS: RefCell<HashMap<(usize, u32), isize>> = RefCell::new(HashMap::new());
    static SUBCLASSED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
}

unsafe extern "system" fn subclass(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    let active = ACTIVE.load(Ordering::Relaxed);
    if msg == WM_SETCURSOR && active != 0 && (lparam.0 & 0xffff) as u32 == HTCLIENT {
        unsafe { SetCursor(Some(HCURSOR(active as *mut core::ffi::c_void))) };
        return LRESULT(1);
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

fn make(img: &CursorImage) -> Option<isize> {
    let side = img.side as i32;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: side,
            // Negative: top-down rows, like the image.
            biHeight: -side,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(color.into());
            return None;
        }
        // 32-bpp icon colors: straight-alpha BGRA.
        std::ptr::copy_nonoverlapping(img.bgra.as_ptr(), bits as *mut u8, img.bgra.len());
        let mask = CreateBitmap(side, side, 1, 1, None);
        let icon = ICONINFO { fIcon: false.into(), xHotspot: img.hot, yHotspot: img.hot, hbmMask: mask, hbmColor: color };
        let cursor = CreateIconIndirect(&icon).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        cursor.map(|c| c.0 as isize)
    }
}

pub fn set(raw: RawWindowHandle, scale: u32, count: Option<usize>, image: &mut dyn FnMut(usize) -> Option<CursorImage>) -> bool {
    let RawWindowHandle::Win32(h) = raw else { return false };
    let hwnd = HWND(h.hwnd.get() as *mut core::ffi::c_void);
    let installed = SUBCLASSED.with(|s| {
        let mut s = s.borrow_mut();
        s.contains(&(hwnd.0 as isize)) || (unsafe { SetWindowSubclass(hwnd, Some(subclass), 0x6e6f_7368, 0) }.as_bool() && s.insert(hwnd.0 as isize))
    });
    if !installed {
        return false;
    }
    match count {
        None => {
            ACTIVE.store(0, Ordering::Relaxed);
            if let Ok(arrow) = unsafe { LoadCursorW(None, IDC_ARROW) } {
                unsafe { SetCursor(Some(arrow)) };
            }
            true
        }
        Some(n) => {
            let Some(cur) = CURSORS.with(|c| {
                let mut c = c.borrow_mut();
                if let Some(cur) = c.get(&(n, scale)) {
                    return Some(*cur);
                }
                let cur = image(n).and_then(|img| make(&img))?;
                c.insert((n, scale), cur);
                Some(cur)
            }) else {
                return false;
            };
            ACTIVE.store(cur, Ordering::Relaxed);
            unsafe { SetCursor(Some(HCURSOR(cur as *mut core::ffi::c_void))) };
            true
        }
    }
}
