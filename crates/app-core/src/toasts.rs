//! One toast at a time (live dx/toast-stack-1125): a document notice that comes while another shows waits in a queue
//! and takes the same slot when that one ends (its ≈5 s, timed by the view from when it shows, or its ✕).
//!
//! A view keeps its one toast slot (`Option<(kind, message)>`, set and cleared as before); [`notice`] puts a
//! document notice there or queues it, and [`ToastQueue::pump`] (from the view's render) brings the next one up once
//! the slot is empty. A view's own message in the slot (a hint) is replaced by a notice, not waited for.

use gpui_kit::*;
use std::collections::VecDeque;

/// A view's queue and toast slot.
pub type Parts<V, K> = fn(&mut V) -> (&mut ToastQueue<K>, &mut Option<(K, String)>);

/// The notices waiting for a view's slot, first in first out.
pub struct ToastQueue<K> {
    waiting: VecDeque<(K, String)>,
    /// The notice the queue last put in the slot.
    shown: Option<(K, String)>,
}

impl<K> Default for ToastQueue<K> {
    fn default() -> Self {
        Self { waiting: VecDeque::new(), shown: None }
    }
}

impl<K: Clone + PartialEq> ToastQueue<K> {
    /// The waiting notices' messages, next first.
    pub fn messages(&self) -> Vec<&str> {
        self.waiting.iter().map(|(_, m)| m.as_str()).collect()
    }

    /// Puts `new` in `slot`, or behind the notice the slot shows; a repeat of one shown or waiting adds nothing.
    fn push(&mut self, slot: &mut Option<(K, String)>, new: (K, String)) {
        if slot.as_ref() == Some(&new) || self.waiting.contains(&new) {
            return;
        }
        if slot.is_some() && *slot == self.shown {
            self.waiting.push_back(new);
            return;
        }
        self.shown = Some(new.clone());
        *slot = Some(new);
    }

    /// The next waiting notice into the empty slot; whether one came up.
    pub fn pump(&mut self, slot: &mut Option<(K, String)>) -> bool {
        if slot.is_some() {
            return false;
        }
        let Some(next) = self.waiting.pop_front() else { return false };
        self.shown = Some(next.clone());
        *slot = Some(next);
        true
    }
}

/// A document notice for `view`'s toast slot (`parts` gives the queue and the slot).
pub fn notice<V: 'static, K: Clone + PartialEq + 'static>(view: &mut V, parts: Parts<V, K>, new: (K, String), cx: &mut Context<V>) {
    let (queue, slot) = parts(view);
    queue.push(slot, new);
    cx.notify();
}

/// The slot's bubble, centred; `None` when it shows nothing.
pub fn column(top: Option<AnyElement>) -> Option<Div> {
    let top = top?;
    Some(div().flex().flex_col().items_center().child(div().debug_selector(|| "toast-top".into()).child(top)))
}

#[cfg(test)]
mod tests {
    use super::ToastQueue;

    /// Live dx/toast-stack-1125: a second notice waits for the first to end and then takes its place; a repeat adds
    /// nothing; a view's own message in the slot is replaced, not waited for.
    #[test]
    fn notices_wait_their_turn_in_one_slot() {
        let mut queue = ToastQueue::default();
        let mut slot = None;
        queue.push(&mut slot, (0, "a".to_string()));
        queue.push(&mut slot, (0, "b".into()));
        queue.push(&mut slot, (1, "c".into()));
        queue.push(&mut slot, (0, "b".into()));
        queue.push(&mut slot, (0, "a".into()));
        assert_eq!((slot.clone().unwrap().1.as_str(), queue.messages()), ("a", vec!["b", "c"]));
        assert!(!queue.pump(&mut slot), "nothing comes up while a shows");
        slot = None;
        assert!(queue.pump(&mut slot));
        assert_eq!((slot.clone().unwrap().1.as_str(), queue.messages()), ("b", vec!["c"]));
        slot = Some((0, "hint".into()));
        queue.push(&mut slot, (0, "d".into()));
        assert_eq!((slot.clone().unwrap().1.as_str(), queue.messages()), ("d", vec!["c"]));
        slot = None;
        assert!(queue.pump(&mut slot));
        assert!(!queue.pump(&mut slot));
        assert_eq!((slot.unwrap().1.as_str(), queue.messages()), ("c", Vec::<&str>::new()));
    }
}
