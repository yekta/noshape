//! Linux backends of the graphics-area cursors (see cursor.rs). X11: an ARGB
//! cursor made with the RENDER extension, or the cursor theme's cursor for a
//! CSS name, set as the window's cursor attribute from our own connection to
//! the same server. Wayland: the patched GPUI Linux platform shows the image
//! or the named theme cursor on its own pointer
//! (vendor/gpui-pre-linux/src/custom_cursor.rs).

use super::{AppCursor, CursorImage};
use raw_window_handle::RawWindowHandle;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use x11rb::connection::Connection;
use x11rb::protocol::render::{ConnectionExt as _, CreatePictureAux, PictType};
use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt as _, CreateGCAux, ImageFormat};
use x11rb::rust_connection::RustConnection;

/// Cursor theme names for a CSS cursor name, most specific first (the names
/// Chrome and GTK look up).
pub(crate) fn theme_names(css: &str) -> &'static [&'static str] {
    match css {
        "move" => &["move", "fleur", "all-scroll", "size_all"],
        "zoom-in" => &["zoom-in"],
        "no-drop" => &["no-drop", "dnd-no-drop", "not-allowed", "crossed_circle"],
        _ => &[],
    }
}

struct X11 {
    conn: RustConnection,
    root: u32,
    format: u32,
    themes: x11rb::cursor::Handle,
    /// The theme's arrow, as GPUI loads it for `CursorStyle::Arrow`.
    arrow: u32,
    cursors: HashMap<(AppCursor, u32), u32>,
}

static STATE: LazyLock<Mutex<Option<X11>>> = LazyLock::new(|| Mutex::new(X11::connect().map_err(|e| log::warn!("app cursors: {e:#}")).ok()));

impl X11 {
    fn connect() -> anyhow::Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let root = conn.setup().roots[screen_num].root;
        let formats = conn.render_query_pict_formats()?.reply()?;
        let format = formats
            .formats
            .iter()
            .find(|f| f.type_ == PictType::DIRECT && f.depth == 32 && f.direct.alpha_mask == 0xff && f.direct.alpha_shift == 24 && f.direct.red_shift == 16 && f.direct.green_shift == 8 && f.direct.blue_shift == 0)
            .map(|f| f.id)
            .ok_or_else(|| anyhow::anyhow!("no ARGB32 picture format"))?;
        let db = x11rb::resource_manager::new_from_default(&conn)?;
        let themes = x11rb::cursor::Handle::new(&conn, screen_num, &db)?.reply()?;
        let arrow = themes.load_cursor(&conn, "left_ptr")?;
        Ok(Self { conn, root, format, themes, arrow, cursors: HashMap::new() })
    }

    fn create(&self, img: &CursorImage) -> anyhow::Result<u32> {
        let c = &self.conn;
        let side = img.side as u16;
        // RENDER cursors take premultiplied ARGB32 (BGRA bytes on LSB-first servers).
        let data = premultiplied(&img.bgra);
        let pix = c.generate_id()?;
        c.create_pixmap(32, pix, self.root, side, side)?;
        let gc = c.generate_id()?;
        c.create_gc(gc, pix, &CreateGCAux::new())?;
        c.put_image(ImageFormat::Z_PIXMAP, pix, gc, side, side, 0, 0, 0, 32, &data)?;
        let pic = c.generate_id()?;
        c.render_create_picture(pic, pix, self.format, &CreatePictureAux::new())?;
        let cur = c.generate_id()?;
        c.render_create_cursor(cur, pic, img.hot.0 as u16, img.hot.1 as u16)?;
        c.render_free_picture(pic)?;
        c.free_gc(gc)?;
        c.free_pixmap(pix)?;
        Ok(cur)
    }

    /// The theme's cursor for a CSS name (0 when the theme has none).
    fn themed(&self, css: &str) -> u32 {
        theme_names(css).iter().map(|n| self.themes.load_cursor(&self.conn, n).unwrap_or(0)).find(|c| *c != 0).unwrap_or(0)
    }
}

pub fn set(raw: RawWindowHandle, scale: u32, cursor: Option<AppCursor>, image: &mut dyn FnMut(AppCursor, bool) -> Option<CursorImage>) -> bool {
    let win = match raw {
        RawWindowHandle::Xcb(h) => h.window.get(),
        RawWindowHandle::Xlib(h) => h.window as u32,
        RawWindowHandle::Wayland(_) => return set_wayland(scale, cursor, image),
        _ => return false,
    };
    let Ok(mut guard) = STATE.lock() else { return false };
    let Some(x) = guard.as_mut() else { return false };
    let id = match cursor {
        // GPUI shows its own cursor again on its next frame.
        None => {
            gpui_linux::custom_cursor::invalidate_style();
            x.arrow
        }
        Some(c) => match x.cursors.get(&(c, scale)) {
            Some(id) => *id,
            None => {
                // A standard cursor from the theme, else (and for Onshape's
                // own images) our drawing.
                let themed = c.css().map(|css| x.themed(css)).unwrap_or(0);
                let id = if themed != 0 { themed } else {
                    let Some(id) = image(c, false).and_then(|img| x.create(&img).ok()) else { return false };
                    id
                };
                x.cursors.insert((c, scale), id);
                id
            }
        },
    };
    x.conn.change_window_attributes(win, &ChangeWindowAttributesAux::new().cursor(id)).is_ok() && x.conn.flush().is_ok()
}

/// Premultiplied ARGB32 in little-endian byte order (BGRA bytes), the
/// format of RENDER cursors and Wayland `Argb8888` buffers.
fn premultiplied(bgra: &[u8]) -> Vec<u8> {
    let mut data = bgra.to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        for ch in &mut px[..3] {
            *ch = ((*ch as u32 * a + 127) / 255) as u8;
        }
    }
    data
}

thread_local! {
    static WAYLAND_IMAGES: std::cell::RefCell<HashMap<(AppCursor, u32), gpui_linux::custom_cursor::CustomCursor>> = std::cell::RefCell::new(HashMap::new());
}

/// Wayland: hand the image, or the CSS name of a standard cursor, to GPUI's
/// pointer (patched platform crate).
fn set_wayland(scale: u32, cursor: Option<AppCursor>, image: &mut dyn FnMut(AppCursor, bool) -> Option<CursorImage>) -> bool {
    use gpui_linux::custom_cursor::{CustomCursor, set_custom_cursor, set_named_cursor};
    let Some(c) = cursor else {
        set_named_cursor(None);
        return set_custom_cursor(None);
    };
    if let Some(css) = c.css() {
        set_custom_cursor(None);
        return set_named_cursor(Some(css));
    }
    set_named_cursor(None);
    let custom = WAYLAND_IMAGES.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(cc) = m.get(&(c, scale)) {
            return Some(cc.clone());
        }
        let img = image(c, false)?;
        let cc = CustomCursor {
            argb: premultiplied(&img.bgra).into(),
            width: img.side,
            height: img.side,
            hot_x: img.hot.0,
            hot_y: img.hot.1,
            // Wayland buffer scales are whole numbers.
            scale: ((scale as f32 / 100.0).round() as i32).max(1),
        };
        m.insert((c, scale), cc.clone());
        Some(cc)
    });
    custom.is_some_and(|cc| set_custom_cursor(Some(cc)))
}
