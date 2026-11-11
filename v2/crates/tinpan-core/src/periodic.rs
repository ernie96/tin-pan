//! When the update check runs again while Tin Pan stays open, and when a version it found may be offered.
//! The window's one-minute timer asks `tick` what to do; the timer itself and the dialog stay in the app.

use std::time::{Duration, Instant};

/// How long Tin Pan waits between update checks while it stays open: a fix reaches people the same day,
/// and 4 checks a day stay far below GitHub's 60 anonymous requests an hour.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often the window's timer calls `tick`: a found version waits at most this long once the app is idle.
pub const TICK_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
pub enum Tick<T> {
    Nothing,
    /// Time for another check (in the background; it says nothing unless it finds something).
    Check,
    /// The app is idle now: offer this version that a check found earlier.
    Offer(T),
}

/// The schedule of the repeated check and the one found version waiting to be offered.
#[derive(Debug)]
pub struct PeriodicCheck<T> {
    every: Duration,
    next_check: Instant,
    waiting: Option<T>,
}

impl<T> PeriodicCheck<T> {
    /// The first repeat comes `every` after `started` (the start-up check has just run).
    pub fn new(started: Instant, every: Duration) -> Self {
        Self { every, next_check: started + every, waiting: None }
    }

    /// `enabled`: the "check at startup" setting, which also covers this check. `idle`: nothing the user is
    /// doing would be interrupted by a dialog.
    pub fn tick(&mut self, now: Instant, enabled: bool, idle: bool) -> Tick<T> {
        if !enabled {
            self.waiting = None;
            return Tick::Nothing;
        }
        if idle && let Some(found) = self.waiting.take() {
            return Tick::Offer(found);
        }
        if now >= self.next_check {
            self.next_check = now + self.every;
            return Tick::Check;
        }
        Tick::Nothing
    }

    /// A check found this version while the app was busy: offer it at the first idle tick. A later find
    /// replaces it.
    pub fn hold(&mut self, found: T) {
        self.waiting = Some(found);
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn checks_every_six_hours_and_not_before() {
        let start = Instant::now();
        let mut periodic = PeriodicCheck::<u32>::new(start, CHECK_EVERY);
        assert_eq!(periodic.tick(start, true, true), Tick::Nothing);
        assert_eq!(periodic.tick(start + 6 * HOUR - TICK_EVERY, true, true), Tick::Nothing);
        assert_eq!(periodic.tick(start + 6 * HOUR, true, true), Tick::Check);
        assert_eq!(periodic.tick(start + 6 * HOUR + TICK_EVERY, true, true), Tick::Nothing);
        assert_eq!(periodic.tick(start + 12 * HOUR, true, true), Tick::Check);
    }

    #[test]
    fn checks_while_busy_but_offers_only_when_idle() {
        let start = Instant::now();
        let mut periodic = PeriodicCheck::new(start, CHECK_EVERY);
        assert_eq!(periodic.tick(start + 6 * HOUR, true, false), Tick::Check);
        periodic.hold(7);
        assert_eq!(periodic.tick(start + 6 * HOUR + TICK_EVERY, true, false), Tick::Nothing);
        assert!(periodic.is_waiting());
        assert_eq!(periodic.tick(start + 6 * HOUR + 2 * TICK_EVERY, true, true), Tick::Offer(7));
        assert_eq!(periodic.tick(start + 6 * HOUR + 3 * TICK_EVERY, true, true), Tick::Nothing);
    }

    #[test]
    fn a_later_find_replaces_the_waiting_one() {
        let start = Instant::now();
        let mut periodic = PeriodicCheck::new(start, CHECK_EVERY);
        periodic.hold(1);
        periodic.hold(2);
        assert_eq!(periodic.tick(start, true, true), Tick::Offer(2));
    }

    #[test]
    fn turned_off_means_no_check_and_no_offer() {
        let start = Instant::now();
        let mut periodic = PeriodicCheck::new(start, CHECK_EVERY);
        periodic.hold(3);
        assert_eq!(periodic.tick(start + 7 * HOUR, false, true), Tick::Nothing);
        assert!(!periodic.is_waiting());
        // Turned back on: the overdue check runs at the next tick.
        assert_eq!(periodic.tick(start + 7 * HOUR, true, true), Tick::Check);
    }
}
