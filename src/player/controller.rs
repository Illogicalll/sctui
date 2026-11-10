use crate::api::Track;
use crate::auth::Token;
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::Ordering,
    mpsc::{self, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

use super::commands::{PlayerCommand, TrackChange};
use super::stream::{Feed, LoadFailure, Shared};
use super::worker::player_loop;

/// Where playback is in the current track: a wall clock that runs while the
/// track is playing, less whatever time the output spent starved of audio.
#[derive(Default)]
pub(crate) struct Position {
    pub elapsed: Duration,
    /// When the clock was last set running; `None` while it is stopped.
    pub last_start: Option<Instant>,
    pub track: Option<Track>,
    /// The audio thread's side of the current track.
    pub feed: Feed,
}

impl Position {
    pub(crate) fn at(&self, now: Instant) -> Duration {
        let running = self
            .last_start
            .map_or(Duration::ZERO, |start| now.saturating_duration_since(start));
        // Silence played while waiting on the network is not progress through
        // the track, or a stall would cut that much off the end.
        (self.elapsed + running).saturating_sub(self.feed.starved())
    }

    pub(crate) fn now(&self) -> Duration {
        self.at(Instant::now())
    }

    /// Sets the clock running. One that is already running is left alone:
    /// restarting it would throw away the time since it started.
    pub(crate) fn start(&mut self, now: Instant) {
        if self.last_start.is_none() {
            self.last_start = Some(now);
        }
    }

    pub(crate) fn stop(&mut self, now: Instant) {
        if let Some(start) = self.last_start.take() {
            self.elapsed += now.saturating_duration_since(start);
        }
    }
}

pub struct Player {
    tx: Sender<PlayerCommand>,
    shared: Shared,
}

impl Player {
    /// Starts the player thread. Fails if no audio output could be opened, so
    /// the caller can say so before the TUI takes over the terminal.
    pub fn new(token: Arc<Mutex<Token>>) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let shared = Shared::new();

        {
            let results = tx.clone();
            let shared = shared.clone();
            thread::spawn(move || player_loop(rx, results, token, shared, ready_tx));
        }
        ready_rx
            .recv()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("the player thread stopped while starting")))?;

        Ok(Self { tx, shared })
    }

    /// Set the crossfade length, in milliseconds (0 turns crossfading off), and
    /// whether it applies to user skips. Takes effect at the next track change.
    pub fn set_crossfade(&self, ms: u64, on_user_skips: bool) {
        let _ = self.tx.send(PlayerCommand::SetCrossfade { ms, on_user_skips });
    }

    /// `change` says whether this is the queue moving on by itself or the user
    /// asking for a different track; the engine needs it to decide whether the
    /// crossfade applies.
    pub fn play(&self, track: Track, change: TrackChange) {
        let _ = self.tx.send(PlayerCommand::Play(track, change));
    }

    pub fn pause(&self) {
        let _ = self.tx.send(PlayerCommand::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(PlayerCommand::Resume);
    }

    pub fn volume_up(&self) {
        let _ = self.tx.send(PlayerCommand::VolumeUp);
    }

    pub fn volume_down(&self) {
        let _ = self.tx.send(PlayerCommand::VolumeDown);
    }

    pub fn fast_forward(&self) {
        let _ = self.tx.send(PlayerCommand::FastForward);
    }

    pub fn rewind(&self) {
        let _ = self.tx.send(PlayerCommand::Rewind);
    }

    pub fn preload_next(&self, track: Track) {
        let _ = self.tx.send(PlayerCommand::PreloadNext(track));
    }

    pub fn is_playing(&self) -> bool {
        self.shared.is_playing.load(Ordering::SeqCst)
    }

    pub fn is_seeking(&self) -> bool {
        self.shared.is_seeking.load(Ordering::SeqCst)
    }

    pub fn elapsed(&self) -> u64 {
        self.shared.position.lock().unwrap().now().as_millis().try_into().unwrap()
    }

    /// The current track has nothing left to play: its stream ran out and
    /// everything decoded from it has been heard, whatever the clock says.
    pub fn track_ended(&self) -> bool {
        self.shared.position.lock().unwrap().feed.ended()
    }

    /// The track that most recently failed to start, if the caller has not
    /// already been told about it.
    pub fn take_load_failure(&self) -> Option<LoadFailure> {
        self.shared.load_failure.lock().unwrap().take()
    }

    pub fn current_track(&self) -> Track {
        self.shared
            .position
            .lock()
            .unwrap()
            .track
            .clone()
            .unwrap_or_else(|| Track {
                title: "No Track Playing - Press <ENTER> on Something to Play!".to_string(),
                artists: "N/A".to_string(),
                duration: "0:00".to_string(),
                duration_ms: 1,
                playback_count: "0".to_string(),
                artwork_url: "".to_string(),
                access: "playable".to_string(),
                track_urn: "".to_string(),
            })
    }

    pub fn get_volume(&self) -> f32 {
        self.shared.volume.get()
    }

    pub fn wave_buffer(&self) -> Arc<Mutex<VecDeque<f32>>> {
        Arc::clone(&self.shared.wave_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_a_clock_that_is_already_running_does_not_rewind_it() {
        let t0 = Instant::now();
        let mut pos = Position::default();
        pos.start(t0);
        // A second "play" 40 s in, as an OS media key sends while playing.
        pos.start(t0 + Duration::from_secs(40));
        assert_eq!(pos.at(t0 + Duration::from_secs(41)), Duration::from_secs(41));
    }

    #[test]
    fn the_clock_only_runs_between_start_and_stop() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let mut pos = Position { elapsed: secs(10), ..Position::default() };
        assert_eq!(pos.at(t0 + secs(5)), secs(10), "stopped");
        pos.start(t0);
        assert_eq!(pos.at(t0 + secs(5)), secs(15));
        pos.stop(t0 + secs(7));
        pos.stop(t0 + secs(9));
        assert_eq!(pos.at(t0 + secs(60)), secs(17));
    }
}
