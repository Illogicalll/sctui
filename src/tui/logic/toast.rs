//! Toasts: short notices of what an action did, or of what failed. At most three at a
//! time, newest last; each lapses on its own.

use std::time::{Duration, Instant};

use super::state::AppState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Ok,
    Err,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub msg: String,
    pub kind: ToastKind,
    /// When it stops being shown.
    pub until: Instant,
}

/// Most toasts on screen at once; a fourth pushes the oldest out.
pub const MAX: usize = 3;
pub const OK_FOR: Duration = Duration::from_secs(3);
pub const ERR_FOR: Duration = Duration::from_secs(6);

/// Add a toast as of `now`. One that repeats the newest toast word for word renews it
/// instead of stacking, so a fetch that keeps failing shows one notice, not three.
pub fn push(toasts: &mut Vec<Toast>, msg: String, kind: ToastKind, now: Instant) {
    let until = now + if kind == ToastKind::Err { ERR_FOR } else { OK_FOR };
    match toasts.last_mut() {
        Some(last) if last.msg == msg && last.kind == kind => last.until = until,
        _ => toasts.push(Toast { msg, kind, until }),
    }
    if toasts.len() > MAX {
        toasts.drain(..toasts.len() - MAX);
    }
}

/// Drop the toasts whose time is up. Run once per loop pass.
pub fn expire(toasts: &mut Vec<Toast>, now: Instant) {
    toasts.retain(|t| t.until > now);
}

impl AppState {
    /// Say what an action did.
    pub fn toast(&mut self, msg: impl Into<String>) {
        push(&mut self.toasts, msg.into(), ToastKind::Ok, Instant::now());
    }

    /// Say what failed. Stays twice as long.
    pub fn toast_err(&mut self, msg: impl Into<String>) {
        push(&mut self.toasts, msg.into(), ToastKind::Err, Instant::now());
    }
}

/// A fetch error as a toast should read. A request that never reached the server is
/// the same news whichever list it was for, so those collapse to one line (and one
/// toast, since an identical newest toast is renewed rather than stacked). Anything
/// else loses its URL, which is long and says nothing the user can act on.
pub fn brief_fetch_error(why: &str) -> String {
    if why.contains("error sending request") || why.contains("operation timed out") {
        return "No connection to SoundCloud · retrying".to_string();
    }
    let mut out = String::new();
    let mut rest = why;
    while let Some(at) = rest.find(" for url (") {
        out.push_str(&rest[..at]);
        rest = rest[at..].find(')').map_or("", |end| &rest[at + end + 1..]);
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn fetch_errors_are_short_and_offline_is_one_message() {
        use super::brief_fetch_error as brief;
        let offline = "Fetch failed: error sending request for url (https://api.soundcloud.com/me/playlists?linked_partitioning=true&limit=40)";
        let other = "Could not load tracks: error sending request for url (https://api.soundcloud.com/playlists/1/tracks)";
        assert_eq!(brief(offline), "No connection to SoundCloud · retrying");
        assert_eq!(brief(offline), brief(other), "one toast, whichever list failed");
        assert_eq!(
            brief("Fetch failed: HTTP status client error (401 Unauthorized) for url (https://api.soundcloud.com/me/likes/tracks?limit=50)"),
            "Fetch failed: HTTP status client error (401 Unauthorized)"
        );
        assert_eq!(brief("Fetch failed: internal error"), "Fetch failed: internal error");
    }

    use super::*;

    #[test]
    fn at_most_three_toasts() {
        let mut state = AppState::default();
        for i in 0..5 {
            state.toast(format!("toast {i}"));
        }
        let shown: Vec<&str> = state.toasts.iter().map(|t| t.msg.as_str()).collect();
        assert_eq!(shown, ["toast 2", "toast 3", "toast 4"], "the oldest are dropped, newest last");
        // The same notice again renews the newest one rather than stacking.
        let before = state.toasts[2].until;
        state.toast("toast 4");
        assert_eq!(state.toasts.len(), 3);
        assert!(state.toasts[2].until >= before);
        state.toast_err("toast 4");
        assert_eq!(state.toasts.last().map(|t| t.kind), Some(ToastKind::Err), "an error is not the same notice");
    }

    #[test]
    fn actions_say_what_they_did() {
        use crate::keymap::Action;
        use crate::tui::logic::input::commands::run_command;
        use crate::tui::render::testkit::{fixture, go};
        let (mut state, mut data, ..) = fixture();
        let run = |action, state: &mut AppState, data: &mut _| {
            run_command(action, state, data);
            let toast = state.toasts.last().expect("a toast");
            (toast.kind, toast.msg.clone())
        };
        let ok = |msg: &str| (ToastKind::Ok, msg.to_string());
        // Library › Likes, first row; the fixture already has two tracks queued.
        assert_eq!(run(Action::AddToQueue, &mut state, &mut data), ok("Added to queue · 3"));
        assert_eq!(run(Action::PlayNext, &mut state, &mut data), ok("Playing next: Song 00"));
        assert_eq!(run(Action::ToggleShuffle, &mut state, &mut data), ok("Shuffle on"));
        assert_eq!(run(Action::ToggleRepeat, &mut state, &mut data), ok("Repeat on"));
        assert_eq!(run(Action::ToggleRepeat, &mut state, &mut data), ok("Repeat off"));
        assert_eq!(run(Action::ToggleLike, &mut state, &mut data), ok("Unliked Song 00"));
        assert_eq!(run(Action::ToggleLike, &mut state, &mut data), ok("Liked Song 00"));
        // A container row is not a track: queueing says so and queues nothing.
        go(&mut state, &mut data, 0, 3, 0);
        let queued = state.manual_queue.len();
        assert_eq!(run(Action::AddToQueue, &mut state, &mut data), (ToastKind::Err, "No playable track selected".to_string()));
        assert_eq!(state.manual_queue.len(), queued);
        assert_eq!(run(Action::ToggleLike, &mut state, &mut data), ok("Followed Friend 0"));
        go(&mut state, &mut data, 0, 1, 0);
        assert_eq!(run(Action::ToggleLike, &mut state, &mut data), ok("Liked Playlist 0"));
    }

    #[test]
    fn ok_expires_after_3s_err_after_6s() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut toasts = Vec::new();
        push(&mut toasts, "liked".into(), ToastKind::Ok, t0);
        push(&mut toasts, "failed".into(), ToastKind::Err, t0);
        expire(&mut toasts, at(2_999));
        assert_eq!(toasts.len(), 2);
        expire(&mut toasts, at(3_000));
        assert_eq!(toasts.iter().map(|t| t.msg.as_str()).collect::<Vec<_>>(), ["failed"]);
        expire(&mut toasts, at(5_999));
        assert_eq!(toasts.len(), 1);
        expire(&mut toasts, at(6_000));
        assert!(toasts.is_empty());
    }
}
