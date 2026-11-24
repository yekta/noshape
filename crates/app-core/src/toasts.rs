//! Toasts that stack: a notice that comes while another shows doesn't replace it; each ✕ closes one (live
//! behaviours "Toasts stack; each ✕ closes one", import-into-doc-1123: one toast per imported file).
//!
//! A view keeps its one toast slot (`Option<(kind, message)>`, set and cleared as before); [`notice`] puts a
//! document notice there and moves the notice it replaces into the view's [`ToastStack`], drawn under the slot's
//! bubble by [`column`].

use crate::doc_model::DocModel;
use crate::os_ui::BubbleKind;
use gpui_kit::*;
use std::time::{Duration, Instant};

/// A view's stack and toast slot.
pub type Parts<V, K> = fn(&mut V) -> (&mut ToastStack<K>, &mut Option<(K, String)>);

/// The notices a view still shows besides its slot's, newest last.
pub struct ToastStack<K> {
    earlier: Vec<(u64, K, String)>,
    /// The notice [`notice`] put in the slot, when, and how long an info notice stays (`None`: until ✕).
    shown: Option<((K, String), Instant, Option<Duration>)>,
    next: u64,
}

impl<K> Default for ToastStack<K> {
    fn default() -> Self {
        Self { earlier: Vec::new(), shown: None, next: 0 }
    }
}

impl<K: Clone + PartialEq> ToastStack<K> {
    /// The stacked notices' messages, newest first (as drawn under the slot).
    pub fn messages(&self) -> Vec<&str> {
        self.earlier.iter().rev().map(|(_, _, m)| m.as_str()).collect()
    }

    pub fn close(&mut self, id: u64) {
        self.earlier.retain(|(i, ..)| *i != id);
    }

    /// Puts `new` in `slot`; the notice the slot showed moves into the stack. Returns the moved notice's id and how
    /// long it has left, when it closes by itself.
    fn push(&mut self, slot: &mut Option<(K, String)>, new: (K, String), stays: Option<Duration>, now: Instant) -> Option<(u64, Duration)> {
        self.earlier.retain(|(_, _, m)| *m != new.1);
        let shown = self.shown.take().filter(|(n, ..)| slot.as_ref() == Some(n) && n.1 != new.1);
        self.shown = Some((new.clone(), now, stays));
        *slot = Some(new);
        let ((kind, msg), at, stays) = shown?;
        let id = self.next;
        self.next += 1;
        self.earlier.push((id, kind, msg));
        stays.map(|s| (id, (at + s).saturating_duration_since(now)))
    }
}

/// A document notice for `view`'s toast slot (`parts` gives the stack and the slot): the notice it replaces stays,
/// stacked, and still closes by itself when its time is up.
pub fn notice<V: 'static, K: Clone + PartialEq + 'static>(view: &mut V, parts: Parts<V, K>, new: (K, String), stays: Option<Duration>, cx: &mut Context<V>) {
    let (stack, slot) = parts(view);
    let Some((id, left)) = stack.push(slot, new, stays, Instant::now()) else {
        cx.notify();
        return;
    };
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(left).await;
        let _ = this.update(cx, |v, cx| {
            parts(v).0.close(id);
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

/// The slot's bubble with the stacked ones under it, newest first, 4 px apart; `None` when neither shows.
pub fn column<V: 'static, K: Clone + PartialEq + 'static>(
    top: Option<AnyElement>,
    stack: &ToastStack<K>,
    view: &Entity<V>,
    parts: Parts<V, K>,
    kind: impl Fn(&K) -> BubbleKind,
    doc: &Entity<DocModel>,
    cx: &App,
) -> Option<Div> {
    if top.is_none() && stack.earlier.is_empty() {
        return None;
    }
    let earlier = stack.earlier.iter().rev().map(|(id, k, msg)| {
        let (view, id) = (view.clone(), *id);
        let close = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            view.update(cx, |v, cx| {
                parts(v).0.close(id);
                cx.notify();
            })
        };
        div().debug_selector(move || format!("toast-stacked-{id}")).child(crate::doc_model::notice_bubble(("toast-stacked", id as usize), kind(k), msg.clone(), doc, close, cx))
    });
    let top = top.map(|t| div().debug_selector(|| "toast-top".into()).child(t));
    Some(div().flex().flex_col().items_center().gap(px(4.)).children(top).children(earlier))
}

#[cfg(test)]
mod tests {
    use super::ToastStack;
    use std::time::{Duration, Instant};

    /// Live: a second notice stacks under the first, each keeps its own time, a repeat doesn't add a bubble, and a
    /// slot cleared or reused by its view (a hint, ✕) has nothing to stack.
    #[test]
    fn notices_stack_and_keep_their_time() {
        let t0 = Instant::now();
        let mut stack = ToastStack::default();
        let mut slot = None;
        assert_eq!(stack.push(&mut slot, (0, "a".into()), Some(Duration::from_secs(5)), t0), None);
        let moved = stack.push(&mut slot, (0, "b".into()), Some(Duration::from_secs(5)), t0 + Duration::from_secs(2));
        assert_eq!(moved.map(|m| m.1), Some(Duration::from_secs(3)));
        assert_eq!((slot.clone().unwrap().1.as_str(), stack.messages()), ("b", vec!["a"]));
        // An error stays until ✕.
        assert_eq!(stack.push(&mut slot, (1, "c".into()), None, t0), Some((1, Duration::from_secs(7))));
        assert_eq!(stack.push(&mut slot, (0, "d".into()), None, t0), None);
        assert_eq!(stack.messages(), ["c", "b", "a"]);
        stack.close(1);
        assert_eq!(stack.messages(), ["c", "a"]);
        // The same message again comes up to the slot once.
        assert_eq!(stack.push(&mut slot, (0, "a".into()), None, t0), None);
        assert_eq!(stack.messages(), ["d", "c"]);
        // The view put its own message in the slot (or closed it): nothing moves.
        slot = Some((0, "hint".into()));
        assert_eq!(stack.push(&mut slot, (0, "e".into()), None, t0), None);
        assert_eq!(stack.messages(), ["d", "c"]);
        slot = None;
        assert_eq!(stack.push(&mut slot, (0, "f".into()), None, t0), None);
        assert_eq!(stack.messages(), ["d", "c"]);
    }
}
