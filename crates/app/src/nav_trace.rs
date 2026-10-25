//! Navigation trace for the input replay tests (.github/nav/, docs/parity/
//! navigation.md): with `NOSHAPE_NAV_TRACE=<file>` every input event the 3D
//! viewport receives and every camera change after it are appended to the
//! file as JSON lines, so a script that injects real OS input (uinput, Windows
//! synthetic pointers, CGEvents) can compare the camera with Onshape's.

use glam::DVec2;
use noshape_render::{Camera, Projection};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

struct Trace {
    file: std::fs::File,
    start: Instant,
}

fn trace() -> Option<&'static Mutex<Trace>> {
    static TRACE: OnceLock<Option<Mutex<Trace>>> = OnceLock::new();
    TRACE
        .get_or_init(|| {
            let path = std::env::var_os("NOSHAPE_NAV_TRACE")?;
            let file = std::fs::OpenOptions::new().create(true).append(true).open(path).ok()?;
            Some(Mutex::new(Trace { file, start: Instant::now() }))
        })
        .as_ref()
}

/// Whether tracing is on (callers skip building the record otherwise).
pub fn enabled() -> bool {
    trace().is_some()
}

fn write(record: Value) {
    let Some(t) = trace() else { return };
    let Ok(mut t) = t.lock() else { return };
    let mut record = record;
    record["t_ms"] = json!(t.start.elapsed().as_secs_f64() * 1000.0);
    // One write per line: the replay drivers append their own lines to the
    // same file (O_APPEND), so a line must not be split across writes.
    let line = format!("{record}\n");
    let _ = t.file.write_all(line.as_bytes());
}

/// An input event as received (`kind` e.g. "scroll", `data` its raw fields).
pub fn input(kind: &str, data: Value) {
    if enabled() {
        write(json!({ "input": kind, "data": data }));
    }
}

/// The camera after a change, with the viewport's size and its origin in the
/// window, in logical pixels.
pub fn camera(camera: &Camera, viewport: DVec2, origin: DVec2) {
    if !enabled() {
        return;
    }
    let v = |d: glam::DVec3| [d.x, d.y, d.z];
    let q = camera.rotation;
    write(json!({
        "camera": {
            "target": v(camera.target),
            "rotation": [q.x, q.y, q.z, q.w],
            "distance": camera.distance,
            "perspective": camera.projection == Projection::Perspective,
            "fov_y": camera.fov_y,
            "eye": v(camera.eye()),
            "up": v(camera.up()),
            "right": v(camera.right()),
            "forward": v(camera.forward()),
            "world_per_pixel": camera.world_per_pixel(viewport.y),
        },
        "viewport": [viewport.x, viewport.y],
        "origin": [origin.x, origin.y],
    }));
}
