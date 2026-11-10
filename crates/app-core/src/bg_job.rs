//! A view's computation off the UI thread, keyed by what it is for (a measurement, mass properties of big imported
//! parts take seconds; drawing must never wait for one): render asks for the key's value, gets `None` while it is
//! under way, and the view redraws when it arrives. Another key replaces the job; a stale value is dropped.

use gpui_kit::{AppContext as _, Context, Task};

/// A job for `key` and its value once it has arrived; dropping it drops the task.
pub struct BgJob<K, V> {
    pub key: K,
    pub value: Option<V>,
    _task: Task<()>,
}

/// The value for `key` from the view's `slot`, or `None` while `work` runs on a background thread for it.
pub fn bg_job<T: 'static, K: PartialEq + Clone + 'static, V: Clone + Send + 'static>(view: &mut T, slot: fn(&mut T) -> &mut Option<BgJob<K, V>>, key: K, work: impl FnOnce() -> V + Send + 'static, cx: &mut Context<T>) -> Option<V> {
    if let Some(job) = slot(view).as_ref()
        && job.key == key
    {
        return job.value.clone();
    }
    let k = key.clone();
    let task = cx.spawn(async move |this, cx| {
        let value = cx.background_spawn(async move { work() }).await;
        let _ = this.update(cx, |view, cx| {
            let Some(job) = slot(view).as_mut().filter(|j| j.key == k) else { return };
            job.value = Some(value);
            cx.notify();
        });
    });
    *slot(view) = Some(BgJob { key, value: None, _task: task });
    None
}
