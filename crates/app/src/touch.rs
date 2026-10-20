//! Onshape's touchscreen gestures (help: "View navigation (Touchscreen)";
//! client: `webgl` bundle's `PointerInputDispatcher` and its recognizers),
//! as a pure state machine fed raw touches. Positions are device pixels, as
//! Onshape's canvas coordinates are; times are explicit so it can be tested.
//!
//! * one finger: tap selects, double tap clears / edits (a double click),
//!   drag rotates (2 turns per viewport diagonal);
//! * two fingers: tap opens the context menu; moving pans, rolls the view
//!   about the centre and pinch-zooms (all three at once, each with its own
//!   tolerance); holding still for 750 ms starts a box selection between the
//!   fingers (window selection) that follows them until one lifts.

use glam::DVec2;
use std::time::Duration;

/// A finger must move more than this (device px, squared: 2 px) before it
/// counts as dragging (`dragThresholdExceeded`).
const DRAG_THRESHOLD_SQ: f64 = 4.0;
/// A finger lifted within this of touching down is a tap.
pub const TAP_MAX_DURATION: Duration = Duration::from_millis(200);
/// Taps are reported this long after the last one (to count double taps).
pub const TAP_REPORT_DELAY: Duration = Duration::from_millis(200);
/// Two fingers held this long start a box selection.
pub const LONG_PRESS: Duration = Duration::from_millis(750);
/// …unless one moves more than this from where the wait began (squared: 10 px).
const LONG_PRESS_SLOP_SQ: f64 = 100.0;
/// Two-finger pan is skipped for a step whose fingers moved very differently
/// (difference of squared moves).
const PAN_MAX_SQ_DIFFERENCE: f64 = 2500.0;
/// Roll and pinch are skipped for a step whose centre moved more than this
/// (squared: 15 px).
const CENTER_MAX_MOVE_SQ: f64 = 225.0;
/// Pinch is skipped for a step whose finger direction turned more than 10°.
fn pinch_max_turn_cos() -> f64 {
    10f64.to_radians().cos()
}

/// What a touch sequence asks the viewport to do.
#[derive(Clone, Debug, PartialEq)]
pub enum TouchAction {
    /// One finger started dragging (rotate, or a tool's left drag), from
    /// where it touched down.
    DragStart { at: DVec2 },
    Drag { delta: DVec2, at: DVec2 },
    DragStop,
    /// Two-finger pan: the first step (pick the pan centre) and every step.
    PanStart { center: DVec2 },
    Pan { center: DVec2, delta: DVec2 },
    /// Two-finger roll about `center` (radians, Onshape's sign).
    Rotate { center: DVec2, angle: f64 },
    /// Pinch: the fingers' distance ratio for this step.
    Pinch { center: DVec2, ratio: f64 },
    /// Two-finger long press: a box between the fingers.
    BoxStart { a: DVec2, b: DVec2 },
    Box { a: DVec2, b: DVec2 },
    BoxStop,
    /// `count` taps of `fingers` fingers centred at `center`.
    Tap { count: usize, fingers: usize, center: DVec2 },
}

#[derive(Clone, Copy, Debug)]
struct Pointer {
    id: u64,
    down_at: DVec2,
    last: DVec2,
    down_time: Duration,
    dragging: bool,
}

/// `pointerDownState` (in touch-down order) and `pointerUpState` (fingers
/// lifted while others stay down; cleared when the last one lifts).
#[derive(Clone, Debug, Default)]
struct State {
    down: Vec<Pointer>,
    up: Vec<Pointer>,
}

#[derive(Clone, Copy, Debug)]
struct TwoPointers {
    p1: DVec2,
    p2: DVec2,
    dir: DVec2,
    center: DVec2,
}

impl State {
    fn two(&self) -> Option<TwoPointers> {
        let [a, b] = self.down.as_slice() else { return None };
        let (p1, p2) = (a.last, b.last);
        Some(TwoPointers { p1, p2, dir: (p2 - p1).normalize_or_zero(), center: (p1 + p2) * 0.5 })
    }

    fn single_dragging(&self) -> Option<&Pointer> {
        match (self.down.as_slice(), self.up.is_empty()) {
            ([p], true) if p.dragging => Some(p),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Tap {
    fingers: usize,
    up_time: Duration,
    position_sum: DVec2,
}

#[derive(Debug, Default)]
pub struct TouchRecognizer {
    state: State,
    taps: Vec<Tap>,
    tap_deadline: Option<Duration>,
    long_press_start: Option<(TwoPointers, Duration)>,
    long_press_active: bool,
    pan_started: bool,
}

impl TouchRecognizer {
    /// A finger touched down.
    pub fn down(&mut self, id: u64, at: DVec2, now: Duration) -> Vec<TouchAction> {
        let prev = self.state.clone();
        self.state.down.retain(|p| p.id != id);
        self.state.down.push(Pointer { id, down_at: at, last: at, down_time: now, dragging: false });
        self.update(&prev, now)
    }

    /// A finger moved.
    pub fn moved(&mut self, id: u64, at: DVec2, now: Duration) -> Vec<TouchAction> {
        let prev = self.state.clone();
        let Some(p) = self.state.down.iter_mut().find(|p| p.id == id) else { return Vec::new() };
        if at.distance_squared(p.down_at) > DRAG_THRESHOLD_SQ {
            p.dragging = true;
        }
        p.last = at;
        self.update(&prev, now)
    }

    /// A finger lifted (or the platform cancelled it).
    pub fn up(&mut self, id: u64, now: Duration) -> Vec<TouchAction> {
        let prev = self.state.clone();
        let Some(i) = self.state.down.iter().position(|p| p.id == id) else { return Vec::new() };
        let p = self.state.down.remove(i);
        if self.state.down.is_empty() {
            self.state = State::default();
        } else {
            self.state.up.push(p);
        }
        self.update(&prev, now)
    }

    /// When [`Self::tick`] next has something to report.
    pub fn next_deadline(&self) -> Option<Duration> {
        let long_press = self.long_press_start.map(|(_, t)| t + LONG_PRESS);
        match (self.tap_deadline, long_press) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Timers: the tap report and the two-finger long press.
    pub fn tick(&mut self, now: Duration) -> Vec<TouchAction> {
        let mut out = Vec::new();
        if let Some((two, start)) = self.long_press_start
            && now >= start + LONG_PRESS
        {
            self.long_press_start = None;
            self.long_press_active = true;
            out.push(TouchAction::BoxStart { a: two.p1, b: two.p2 });
        }
        if self.tap_deadline.is_some_and(|t| now >= t) {
            self.tap_deadline = None;
            let taps = std::mem::take(&mut self.taps);
            if let Some(first) = taps.first()
                && taps.iter().all(|t| t.fingers == first.fingers)
            {
                let last = taps.last().expect("taps");
                out.push(TouchAction::Tap { count: taps.len(), fingers: first.fingers, center: last.position_sum / first.fingers as f64 });
            }
        }
        out
    }

    /// `updateGestures`: every recognizer sees the state before and after.
    fn update(&mut self, prev: &State, now: Duration) -> Vec<TouchAction> {
        let mut out = Vec::new();
        let new = self.state.clone();
        // Single-finger drag.
        match (prev.single_dragging(), new.single_dragging()) {
            (None, Some(p)) => out.push(TouchAction::DragStart { at: p.down_at }),
            (Some(a), Some(b)) => out.push(TouchAction::Drag { delta: b.last - a.last, at: b.last }),
            (Some(_), None) => out.push(TouchAction::DragStop),
            (None, None) => {}
        }
        // Taps: fingers lifted quickly; fingers down before the previous tap
        // ended belong to it.
        let mut tapped = false;
        for p in prev.down.iter().filter(|p| !new.down.iter().any(|q| q.id == p.id)) {
            if now.saturating_sub(p.down_time) < TAP_MAX_DURATION {
                tapped = true;
                match self.taps.last_mut() {
                    Some(t) if p.down_time < t.up_time => {
                        t.fingers += 1;
                        t.position_sum += p.down_at;
                    }
                    _ => self.taps.push(Tap { fingers: 1, up_time: now, position_sum: p.down_at }),
                }
            }
        }
        if tapped {
            self.tap_deadline = Some(now + TAP_REPORT_DELAY);
        }
        let (before, after) = (prev.two(), new.two());
        // Two-finger pan.
        let started = std::mem::take(&mut self.pan_started);
        if let (Some(n), Some(s)) = (before, after) {
            let (o, a) = (s.p1.distance_squared(n.p1), s.p2.distance_squared(n.p2));
            if (a - o).abs() <= PAN_MAX_SQ_DIFFERENCE {
                self.pan_started = true;
                out.push(if started { TouchAction::Pan { center: n.center, delta: s.center - n.center } } else { TouchAction::PanStart { center: n.center } });
                if !started {
                    out.push(TouchAction::Pan { center: n.center, delta: s.center - n.center });
                }
            }
        }
        // Two-finger roll and pinch (centre nearly still).
        if let (Some(n), Some(s)) = (before, after)
            && n.center.distance_squared(s.center) <= CENTER_MAX_MOVE_SQ
        {
            let cross = s.dir.x * n.dir.y - s.dir.y * n.dir.x;
            out.push(TouchAction::Rotate { center: n.center, angle: cross.atan2(s.dir.dot(n.dir)) });
            let r = n.p1.distance(n.p2);
            if n.dir.dot(s.dir) >= pinch_max_turn_cos() && r > 0.0 {
                let ratio = s.p1.distance(s.p2) / r;
                if (1.0 - ratio).abs() >= 1e-4 {
                    out.push(TouchAction::Pinch { center: s.center, ratio });
                }
            }
        }
        // Two-finger long press → box.
        match after {
            Some(s) if self.long_press_active => out.push(TouchAction::Box { a: s.p1, b: s.p2 }),
            Some(s) => match self.long_press_start {
                None => self.long_press_start = Some((s, now)),
                Some((start, _)) => {
                    if start.p1.distance_squared(s.p1) > LONG_PRESS_SLOP_SQ || start.p2.distance_squared(s.p2) > LONG_PRESS_SLOP_SQ {
                        self.long_press_start = None;
                    }
                }
            },
            None => {
                if std::mem::take(&mut self.long_press_active) {
                    out.push(TouchAction::BoxStop);
                }
                self.long_press_start = None;
            }
        }
        out
    }

    /// Whether a box selection is in progress (other gestures are ignored).
    pub fn boxing(&self) -> bool {
        self.long_press_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn v(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    #[test]
    fn single_tap_and_double_tap() {
        let mut r = TouchRecognizer::default();
        assert!(r.down(1, v(100.0, 100.0), ms(0)).is_empty());
        assert!(r.up(1, ms(80)).is_empty());
        // Reported only after the 200 ms wait.
        assert!(r.tick(ms(200)).is_empty());
        assert_eq!(r.tick(ms(280)), vec![TouchAction::Tap { count: 1, fingers: 1, center: v(100.0, 100.0) }]);
        // Double tap.
        r.down(2, v(50.0, 60.0), ms(1000));
        r.up(2, ms(1060));
        r.down(3, v(51.0, 61.0), ms(1150));
        r.up(3, ms(1200));
        assert!(r.tick(ms(1300)).is_empty());
        assert_eq!(r.tick(ms(1400)), vec![TouchAction::Tap { count: 2, fingers: 1, center: v(51.0, 61.0) }]);
        // A slow press is no tap.
        r.down(4, v(0.0, 0.0), ms(2000));
        r.up(4, ms(2300));
        assert!(r.tick(ms(3000)).is_empty());
    }

    #[test]
    fn two_finger_tap_is_one_tap_of_two() {
        let mut r = TouchRecognizer::default();
        r.down(1, v(100.0, 100.0), ms(0));
        r.down(2, v(200.0, 100.0), ms(20));
        r.up(1, ms(90));
        r.up(2, ms(100));
        assert_eq!(r.tick(ms(300)), vec![TouchAction::Tap { count: 1, fingers: 2, center: v(150.0, 100.0) }]);
    }

    #[test]
    fn one_finger_drag_starts_past_two_pixels() {
        let mut r = TouchRecognizer::default();
        r.down(1, v(10.0, 10.0), ms(0));
        assert!(r.moved(1, v(11.0, 11.0), ms(10)).is_empty());
        assert_eq!(r.moved(1, v(13.0, 10.0), ms(20)), vec![TouchAction::DragStart { at: v(10.0, 10.0) }]);
        assert_eq!(r.moved(1, v(20.0, 12.0), ms(30)), vec![TouchAction::Drag { delta: v(7.0, 2.0), at: v(20.0, 12.0) }]);
        assert_eq!(r.up(1, ms(40)), vec![TouchAction::DragStop]);
    }

    #[test]
    fn two_finger_pan_roll_and_pinch() {
        let mut r = TouchRecognizer::default();
        r.down(1, v(100.0, 100.0), ms(0));
        r.down(2, v(200.0, 100.0), ms(5));
        // Each event moves one finger: the first step starts the pan (and
        // pans by the centre's move), the next one pans.
        let a = r.moved(1, v(105.0, 100.0), ms(300));
        assert_eq!(a[0], TouchAction::PanStart { center: v(150.0, 100.0) });
        assert_eq!(a[1], TouchAction::Pan { center: v(150.0, 100.0), delta: v(2.5, 0.0) });
        let a = r.moved(2, v(205.0, 100.0), ms(310));
        assert_eq!(a[0], TouchAction::Pan { center: v(152.5, 100.0), delta: v(2.5, 0.0) });
        // Spread: pinch with the distance ratio.
        let a = r.moved(2, v(215.0, 100.0), ms(320));
        assert!(a.iter().any(|x| matches!(x, TouchAction::Pinch { ratio, .. } if (ratio - 110.0 / 100.0).abs() < 1e-9)), "{a:?}");
        // Turn the second finger a little clockwise (y is down; the centre
        // moves under 15 px): a roll, negative in Onshape's sign.
        let a = r.moved(2, v(214.0, 110.0), ms(330));
        assert!(a.iter().any(|x| matches!(x, TouchAction::Rotate { angle, .. } if *angle < -0.05)), "{a:?}");
        // A big jump of the centre: no roll for that step.
        let a = r.moved(2, v(300.0, 200.0), ms(340));
        assert!(!a.iter().any(|x| matches!(x, TouchAction::Rotate { .. })), "{a:?}");
    }

    #[test]
    fn two_finger_long_press_boxes_until_a_finger_lifts() {
        let mut r = TouchRecognizer::default();
        r.down(1, v(100.0, 100.0), ms(0));
        r.down(2, v(300.0, 250.0), ms(10));
        assert_eq!(r.next_deadline(), Some(ms(760)));
        assert!(r.tick(ms(700)).is_empty());
        assert_eq!(r.tick(ms(760)), vec![TouchAction::BoxStart { a: v(100.0, 100.0), b: v(300.0, 250.0) }]);
        assert!(r.boxing());
        let a = r.moved(2, v(350.0, 260.0), ms(800));
        assert!(a.contains(&TouchAction::Box { a: v(100.0, 100.0), b: v(350.0, 260.0) }), "{a:?}");
        assert!(r.up(1, ms(900)).contains(&TouchAction::BoxStop));
        assert!(!r.boxing());
    }

    #[test]
    fn moving_fingers_restart_the_long_press_wait() {
        let mut r = TouchRecognizer::default();
        r.down(1, v(100.0, 100.0), ms(0));
        r.down(2, v(300.0, 100.0), ms(10));
        r.moved(2, v(320.0, 100.0), ms(400)); // > 10 px: wait cancelled
        assert_eq!(r.next_deadline(), None);
        r.moved(2, v(321.0, 100.0), ms(410)); // restarts from here
        assert_eq!(r.next_deadline(), Some(ms(1160)));
    }

    #[test]
    fn lifting_one_of_two_fingers_does_not_rotate() {
        let mut r = TouchRecognizer::default();
        r.down(1, v(100.0, 100.0), ms(0));
        r.down(2, v(200.0, 100.0), ms(5));
        r.up(2, ms(400));
        let a = r.moved(1, v(150.0, 100.0), ms(450));
        assert!(!a.iter().any(|x| matches!(x, TouchAction::DragStart { .. } | TouchAction::Drag { .. })), "{a:?}");
    }
}
