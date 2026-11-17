//! When each of the library's paged lists (and the feed) may be asked for its next page.
//!
//! The loop wants the next page of whatever is on screen on every tick. Without this
//! that was a new thread and a new request every 200ms, whether or not the last one
//! had come back, had failed, or the list had ended.

use std::time::{Duration, Instant};

use crate::api::List;

/// Wait after a first failure; doubles with each further one, up to `MAX_RETRY`.
const FIRST_RETRY: Duration = Duration::from_secs(2);
const MAX_RETRY: Duration = Duration::from_secs(60);

#[derive(Default, Clone, Copy)]
struct Pager {
    in_flight: bool,
    exhausted: bool,
    /// Failures in a row.
    failures: u32,
    not_before: Option<Instant>,
}

/// The `bool` is a hold: no fetch starts while it is set (see `hold`).
#[derive(Default)]
pub struct Pagers([Pager; 5], bool);

impl Pagers {
    /// Whether to fetch `list`'s next page now. If so it counts as in flight until
    /// `settled`. Never while a fetch is out, after a failure until its backoff has
    /// passed, or once the list has ended.
    pub fn start(&mut self, list: List, now: Instant) -> bool {
        let pager = &mut self.0[list as usize];
        if self.1 || pager.in_flight || pager.exhausted || pager.not_before.is_some_and(|at| now < at) {
            return false;
        }
        pager.in_flight = true;
        true
    }

    /// Start nothing more, so the fetches already out can drain before a refresh: a
    /// page that lands after the lists start over would be a page of the old ones.
    pub fn hold(&mut self) {
        self.1 = true;
    }

    /// Held, and every fetch that was out has settled.
    pub fn drained(&self) -> bool {
        self.1 && self.0.iter().all(|pager| !pager.in_flight)
    }

    /// The fetch `start` allowed came back: `Some(more)` with whether pages remain, or
    /// `None` if it failed. Returns true for the first failure of a streak, the one
    /// worth telling the user about.
    pub fn settled(&mut self, list: List, outcome: Option<bool>, now: Instant) -> bool {
        let pager = &mut self.0[list as usize];
        pager.in_flight = false;
        match outcome {
            Some(more) => {
                *pager = Pager { exhausted: !more, ..Pager::default() };
                false
            }
            None => {
                let delay = FIRST_RETRY.saturating_mul(1 << pager.failures.min(10)).min(MAX_RETRY);
                pager.failures += 1;
                pager.not_before = Some(now + delay);
                pager.failures == 1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #11 + T15: one fetch per list at a time, backing off while it fails.
    #[test]
    fn a_list_is_not_fetched_while_in_flight_backing_off_or_ended() {
        let t0 = Instant::now();
        let secs = |s| t0 + Duration::from_secs(s);
        let mut pagers = Pagers::default();

        assert!(pagers.start(List::Likes, t0));
        assert!(!pagers.start(List::Likes, secs(30)), "still out: no second fetch, however long it takes");
        assert!(pagers.start(List::Feed, t0), "lists are independent");

        // Failures: 2s, 4s, 8s ... capped at 60s; only the first one is announced.
        let mut now = t0;
        for (nth, wait) in [2, 4, 8, 16, 32, 60, 60].into_iter().enumerate() {
            assert_eq!(pagers.settled(List::Likes, None, now), nth == 0);
            assert!(!pagers.start(List::Likes, now + Duration::from_secs(wait) - Duration::from_millis(1)));
            now += Duration::from_secs(wait);
            assert!(pagers.start(List::Likes, now), "retried after {wait}s");
        }

        // A success clears the streak: the next page goes at once, and a new failure
        // starts again at 2s and is announced again.
        assert!(!pagers.settled(List::Likes, Some(true), now));
        assert!(pagers.start(List::Likes, now));
        assert!(pagers.settled(List::Likes, None, now));
        assert!(pagers.start(List::Likes, now + Duration::from_secs(2)));

        // Held for a refresh: nothing starts, and it is drained once what was out is back.
        assert!(!pagers.drained(), "not held");
        pagers.hold();
        assert!(!pagers.start(List::Albums, now) && !pagers.drained(), "Feed and Likes are still out");
        pagers.settled(List::Feed, Some(true), now);

        // The last page: nothing more is ever fetched.
        assert!(!pagers.settled(List::Likes, Some(false), now));
        assert!(!pagers.start(List::Likes, now + Duration::from_secs(3600)));
        assert!(pagers.drained());
    }
}
