//! Meridian: what a finger does in an inventory. The game replays the events as mouse input, so a
//! tap clicks, a drag carries a stack, a long press right-clicks (take half, place one) and, on the
//! creative item grid, a vertical drag scrolls. No game state or window in here: data in, events out.
use std::time::{Duration, Instant};

/// A finger held this long without moving right-clicks.
const LONG_PRESS: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuButton {
    Left,
    Right,
}

/// One mouse event for the game to replay, in the surface's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuEvent {
    Move([f64; 2]),
    Press(MenuButton),
    Release(MenuButton),
    /// Scroll by the finger's own vertical movement.
    Wheel(f64),
}

#[derive(Clone, Copy, Debug, Default)]
enum State {
    #[default]
    Idle,
    /// Down, not yet a tap, drag, scroll or long press.
    Pending {
        id: u64,
        start: [f64; 2],
        since: Instant,
        slop: f64,
        may_scroll: bool,
    },
    /// Dragging with the left button held.
    Pressed {
        id: u64,
    },
    Scrolling {
        id: u64,
        last_y: f64,
    },
    /// Already acted (a long press): its moves and lift do nothing.
    Spent {
        id: u64,
    },
}

/// The one finger an inventory follows at a time.
#[derive(Default)]
pub struct MenuFinger {
    state: State,
}

impl MenuFinger {
    /// A finger lands at `at`. `slop` is how far it may move and still be a tap or a long press;
    /// `may_scroll` says it landed on the creative item grid with nothing carried. The caller moves
    /// the cursor to `at` first. A second finger is ignored.
    pub fn down(
        &mut self,
        id: u64,
        at: [f64; 2],
        now: Instant,
        slop: f64,
        may_scroll: bool,
    ) -> Vec<MenuEvent> {
        if matches!(self.state, State::Idle) {
            self.state = State::Pending {
                id,
                start: at,
                since: now,
                slop,
                may_scroll,
            };
        }
        Vec::new()
    }

    pub fn moved(&mut self, id: u64, at: [f64; 2], _now: Instant) -> Vec<MenuEvent> {
        match self.state {
            State::Pending {
                id: own,
                start,
                slop,
                may_scroll,
                ..
            } if own == id && (at[0] - start[0]).hypot(at[1] - start[1]) > slop => {
                if may_scroll && (at[1] - start[1]).abs() > (at[0] - start[0]).abs() {
                    self.state = State::Scrolling { id, last_y: at[1] };
                    vec![MenuEvent::Wheel(at[1] - start[1])]
                } else {
                    self.state = State::Pressed { id };
                    vec![
                        MenuEvent::Move(start),
                        MenuEvent::Press(MenuButton::Left),
                        MenuEvent::Move(at),
                    ]
                }
            }
            State::Pressed { id: own } if own == id => vec![MenuEvent::Move(at)],
            State::Scrolling { id: own, last_y } if own == id => {
                self.state = State::Scrolling { id, last_y: at[1] };
                vec![MenuEvent::Wheel(at[1] - last_y)]
            }
            _ => Vec::new(),
        }
    }

    pub fn up(&mut self, id: u64, at: [f64; 2], _now: Instant) -> Vec<MenuEvent> {
        let events = match self.state {
            State::Pending { id: own, .. } if own == id => {
                vec![
                    MenuEvent::Press(MenuButton::Left),
                    MenuEvent::Release(MenuButton::Left),
                ]
            }
            State::Pressed { id: own } if own == id => {
                vec![MenuEvent::Move(at), MenuEvent::Release(MenuButton::Left)]
            }
            State::Scrolling { id: own, .. } | State::Spent { id: own } if own == id => Vec::new(),
            _ => return Vec::new(),
        };
        self.state = State::Idle;
        events
    }

    /// Fires the long press once the finger has held still long enough.
    pub fn tick(&mut self, now: Instant) -> Vec<MenuEvent> {
        match self.state {
            State::Pending { id, since, .. } if now.duration_since(since) >= LONG_PRESS => {
                self.state = State::Spent { id };
                vec![
                    MenuEvent::Press(MenuButton::Right),
                    MenuEvent::Release(MenuButton::Right),
                ]
            }
            _ => Vec::new(),
        }
    }

    /// Whether this finger belongs to the menu gesture, so the game keeps its events.
    pub fn owns(&self, id: u64) -> bool {
        match self.state {
            State::Idle => false,
            State::Pending { id: own, .. }
            | State::Pressed { id: own }
            | State::Scrolling { id: own, .. }
            | State::Spent { id: own } => own == id,
        }
    }

    /// Some finger has the menu: another one landing is ignored.
    pub fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// A long press may still fire: the game keeps ticking.
    pub fn pending(&self) -> bool {
        matches!(self.state, State::Pending { .. })
    }

    /// Drops the finger without any event (the screen closed).
    pub fn reset(&mut self) {
        self.state = State::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::MenuButton::*;
    use super::MenuEvent::*;
    use super::*;

    const SLOP: f64 = 20.0;

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn a_tap_clicks_left() {
        let (t, mut f) = (Instant::now(), MenuFinger::default());
        assert!(f.down(1, [50.0, 60.0], t, SLOP, false).is_empty());
        assert!(f.pending() && f.owns(1) && !f.owns(2));
        assert!(f.moved(1, [55.0, 62.0], ms(t, 50)).is_empty());
        assert_eq!(
            f.up(1, [55.0, 62.0], ms(t, 80)),
            [Press(Left), Release(Left)]
        );
        assert!(!f.owns(1) && !f.pending());
    }

    #[test]
    fn a_long_press_right_clicks_once_and_spends_the_finger() {
        let (t, mut f) = (Instant::now(), MenuFinger::default());
        f.down(1, [0.0; 2], t, SLOP, false);
        assert!(f.tick(ms(t, 399)).is_empty() && f.pending());
        assert_eq!(f.tick(ms(t, 400)), [Press(Right), Release(Right)]);
        assert!(!f.pending() && f.owns(1));
        assert!(f.tick(ms(t, 900)).is_empty());
        assert!(f.moved(1, [300.0, 0.0], ms(t, 950)).is_empty());
        assert!(f.up(1, [300.0, 0.0], ms(t, 1000)).is_empty());
        assert!(!f.owns(1));
    }

    #[test]
    fn a_drag_presses_where_it_started_and_releases_where_it_lifts() {
        let (t, mut f) = (Instant::now(), MenuFinger::default());
        f.down(1, [10.0, 10.0], t, SLOP, false);
        assert_eq!(
            f.moved(1, [40.0, 10.0], ms(t, 30)),
            [Move([10.0, 10.0]), Press(Left), Move([40.0, 10.0])]
        );
        assert!(!f.pending());
        assert_eq!(f.moved(1, [90.0, 20.0], ms(t, 60)), [Move([90.0, 20.0])]);
        assert!(f.tick(ms(t, 2000)).is_empty());
        assert_eq!(
            f.up(1, [100.0, 30.0], ms(t, 90)),
            [Move([100.0, 30.0]), Release(Left)]
        );
    }

    #[test]
    fn a_vertical_drag_on_the_grid_scrolls_with_the_fingers_sign() {
        let (t, mut f) = (Instant::now(), MenuFinger::default());
        f.down(1, [10.0, 100.0], t, SLOP, true);
        assert_eq!(f.moved(1, [12.0, 70.0], ms(t, 30)), [Wheel(-30.0)]);
        assert_eq!(f.moved(1, [12.0, 50.0], ms(t, 60)), [Wheel(-20.0)]);
        assert_eq!(f.moved(1, [12.0, 60.0], ms(t, 90)), [Wheel(10.0)]);
        assert!(f.up(1, [12.0, 60.0], ms(t, 120)).is_empty());
        // A sideways drag on the grid is still a drag, and without the grid so is a vertical one.
        f.down(2, [10.0, 100.0], t, SLOP, true);
        assert_eq!(f.moved(2, [60.0, 100.0], ms(t, 30))[1], Press(Left));
        f.up(2, [60.0, 100.0], ms(t, 40));
        f.down(3, [10.0, 100.0], t, SLOP, false);
        assert_eq!(f.moved(3, [10.0, 40.0], ms(t, 30))[1], Press(Left));
    }

    #[test]
    fn other_fingers_never_press_or_release() {
        let (t, mut f) = (Instant::now(), MenuFinger::default());
        assert!(f.up(9, [0.0; 2], t).is_empty());
        f.down(1, [0.0; 2], t, SLOP, false);
        assert!(f.down(2, [5.0, 5.0], t, SLOP, false).is_empty());
        assert!(f.moved(2, [99.0, 99.0], ms(t, 10)).is_empty());
        assert!(f.up(2, [99.0, 99.0], ms(t, 20)).is_empty());
        assert!(f.owns(1) && !f.owns(2));
        f.moved(1, [50.0, 0.0], ms(t, 30));
        assert!(f.up(2, [99.0, 99.0], ms(t, 40)).is_empty());
        assert_eq!(f.up(1, [50.0, 0.0], ms(t, 50))[1], Release(Left));
        f.reset();
        assert!(!f.owns(1));
    }
}
