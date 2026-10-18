use crate::linux::Globals;
use crate::linux::{DEFAULT_CURSOR_ICON_NAME, log_cursor_icon_warning};
use anyhow::{Context as _, anyhow};
use gpui_util::ResultExt;

use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::{wl_pointer::WlPointer, wl_shm, wl_shm::WlShm};
use wayland_client::{Connection, QueueHandle};

use super::client::WaylandClientStatePtr;
use wayland_cursor::{CursorImageBuffer, CursorTheme};

pub(crate) struct Cursor {
    loaded_theme: Option<LoadedTheme>,
    size: u32,
    scaled_size: u32,
    surface: WlSurface,
    shm: WlShm,
    connection: Connection,
    // NOSHAPE PATCH: the application image cursor's buffer (custom_cursor.rs).
    qh: QueueHandle<WaylandClientStatePtr>,
    custom_buffer: Option<WlBuffer>,
}

pub(crate) struct LoadedTheme {
    theme: CursorTheme,
    name: Option<String>,
    scaled_size: u32,
}

impl Drop for Cursor {
    fn drop(&mut self) {
        if let Some(buffer) = self.custom_buffer.take() {
            buffer.destroy();
        }
        self.loaded_theme.take();
        self.surface.destroy();
    }
}

impl Cursor {
    pub fn new(connection: &Connection, globals: &Globals, size: u32) -> Self {
        let mut this = Self {
            loaded_theme: None,
            size,
            scaled_size: size,
            surface: globals.compositor.create_surface(&globals.qh, ()),
            shm: globals.shm.clone(),
            connection: connection.clone(),
            qh: globals.qh.clone(),
            custom_buffer: None,
        };
        this.set_theme_internal(None);
        this
    }

    fn set_theme_internal(&mut self, theme_name: Option<String>) {
        if let Some(loaded_theme) = self.loaded_theme.as_ref()
            && loaded_theme.name == theme_name
            && loaded_theme.scaled_size == self.scaled_size
        {
            return;
        }
        let result = if let Some(theme_name) = theme_name.as_ref() {
            CursorTheme::load_from_name(
                &self.connection,
                self.shm.clone(),
                theme_name,
                self.scaled_size,
            )
        } else {
            CursorTheme::load(&self.connection, self.shm.clone(), self.scaled_size)
        };
        if let Some(theme) = result
            .context("Wayland: Failed to load cursor theme")
            .log_err()
        {
            self.loaded_theme = Some(LoadedTheme {
                theme,
                name: theme_name,
                scaled_size: self.scaled_size,
            });
        }
    }

    pub fn set_theme(&mut self, theme_name: String) {
        self.set_theme_internal(Some(theme_name));
    }

    fn set_scaled_size(&mut self, scaled_size: u32) {
        self.scaled_size = scaled_size;
        let theme_name = self
            .loaded_theme
            .as_ref()
            .and_then(|loaded_theme| loaded_theme.name.clone());
        self.set_theme_internal(theme_name);
    }

    pub fn set_size(&mut self, size: u32) {
        self.size = size;
        self.set_scaled_size(size);
    }

    /// NOSHAPE PATCH: show an application image cursor (custom_cursor.rs)
    /// through a `wl_shm` buffer on the cursor surface.
    pub fn set_custom(&mut self, wl_pointer: &WlPointer, serial_id: u32, cursor: &crate::custom_cursor::CustomCursor) {
        use std::io::Write as _;
        use std::os::fd::{AsFd as _, FromRawFd as _};
        let (w, h) = (cursor.width as i32, cursor.height as i32);
        let len = cursor.argb.len();
        if len != (w * h * 4) as usize || len == 0 {
            return;
        }
        #[cfg(target_os = "linux")]
        let fd = unsafe { libc::memfd_create(c"gpui-custom-cursor".as_ptr(), libc::MFD_CLOEXEC) };
        #[cfg(not(target_os = "linux"))]
        let fd = unsafe { libc::shm_open(libc::SHM_ANON, libc::O_RDWR | libc::O_CREAT, 0o600) };
        if fd < 0 {
            log::warn!("wayland: no shared memory for the custom cursor");
            return;
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        if file.write_all(&cursor.argb).is_err() {
            return;
        }
        let pool = self.shm.create_pool(file.as_fd(), len as i32, &self.qh, ());
        let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, &self.qh, ());
        pool.destroy();
        let scale = cursor.scale.max(1);
        self.surface.set_buffer_scale(scale);
        wl_pointer.set_cursor(serial_id, Some(&self.surface), cursor.hot_x as i32 / scale, cursor.hot_y as i32 / scale);
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage(0, 0, w, h);
        self.surface.commit();
        if let Some(old) = self.custom_buffer.replace(buffer) {
            old.destroy();
        }
    }

    pub fn set_icon(
        &mut self,
        wl_pointer: &WlPointer,
        serial_id: u32,
        cursor_icon_names: &[&str],
        scale: i32,
    ) {
        self.set_scaled_size(self.size * scale as u32);

        let Some(loaded_theme) = &mut self.loaded_theme else {
            log::warn!("Wayland: Unable to load cursor themes");
            return;
        };
        let theme = &mut loaded_theme.theme;

        let buffer: &CursorImageBuffer;
        'outer: {
            for cursor_icon_name in cursor_icon_names {
                if let Some(cursor) = theme.get_cursor(cursor_icon_name) {
                    buffer = &cursor[0];
                    break 'outer;
                }
            }

            if let Some(cursor) = theme.get_cursor(DEFAULT_CURSOR_ICON_NAME) {
                buffer = &cursor[0];
                log_cursor_icon_warning(anyhow!(
                    "wayland: Unable to get cursor icon {:?}. \
                    Using default cursor icon: '{}'",
                    cursor_icon_names,
                    DEFAULT_CURSOR_ICON_NAME
                ));
            } else {
                log_cursor_icon_warning(anyhow!(
                    "wayland: Unable to fallback on default cursor icon '{}' for theme '{}'",
                    DEFAULT_CURSOR_ICON_NAME,
                    loaded_theme.name.as_deref().unwrap_or("default")
                ));
                return;
            }
        }

        let (width, height) = buffer.dimensions();
        let (hot_x, hot_y) = buffer.hotspot();

        self.surface.set_buffer_scale(scale);

        wl_pointer.set_cursor(
            serial_id,
            Some(&self.surface),
            hot_x as i32 / scale,
            hot_y as i32 / scale,
        );

        self.surface.attach(Some(buffer), 0, 0);
        self.surface.damage(0, 0, width as i32, height as i32);
        self.surface.commit();
    }
}
