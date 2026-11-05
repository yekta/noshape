//! NOSHAPE PATCH (agent psui): the GPU time of each labelled command buffer, from Metal's `GPUStartTime` /
//! `GPUEndTime` (devices without timestamp queries, such as CI's paravirtualized Apple GPU, still report them).
//! wgpu-core records every render and compute pass in a command buffer of its own, labelled with the pass's label,
//! so these are per-pass times. On with `NOSHAPE_PERF_PASSES=1`; the application takes them with [`take`].

use std::{string::{String, ToString}, sync::OnceLock, vec::Vec};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::MTLCommandBuffer;

static TIMES: parking_lot::Mutex<Vec<(String, f64)>> = parking_lot::Mutex::new(Vec::new());

/// Whether pass timing is on.
pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("NOSHAPE_PERF_PASSES").is_ok_and(|v| v == "1"))
}

/// Record `raw`'s GPU time when it completes, under its label (wgpu's own buffers are left out).
pub(super) fn watch(raw: &Retained<ProtocolObject<dyn MTLCommandBuffer>>) {
    let Some(name) = raw.label().map(|l| l.to_string()) else { return };
    if name.starts_with("(wgpu") {
        return;
    }
    let block = block2::RcBlock::new(move |cb: core::ptr::NonNull<ProtocolObject<dyn MTLCommandBuffer>>| {
        // SAFETY: Metal passes the completed command buffer, alive for the call.
        let cb = unsafe { cb.as_ref() };
        // `CFTimeInterval` seconds (sent by selector: the typed binding needs a feature this crate doesn't enable).
        let (start, end): (f64, f64) = unsafe { (objc2::msg_send![cb, GPUStartTime], objc2::msg_send![cb, GPUEndTime]) };
        let ms = (end - start) * 1e3;
        TIMES.lock().push((name.clone(), ms));
    });
    // SAFETY: the block is copied by Metal and called once the buffer completes.
    unsafe { raw.addCompletedHandler(block2::RcBlock::as_ptr(&block)) };
}

/// The (pass, GPU milliseconds) of the buffers completed since the last call.
pub fn take() -> Vec<(String, f64)> {
    core::mem::take(&mut *TIMES.lock())
}
