use crate::auth::Token;
use rodio::Sink;
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::Receiver,
};
use std::time::{Duration, Instant};

use super::Position;
use super::commands::{PlayerCommand, TrackChange};
use super::stream::{PlaybackEngine, open_output_stream};

/// Pulls the next command to act on. Spam-skipping next/prev queues a burst of `Play`
/// commands faster than one can load, so a run of them already waiting in the channel is
/// collapsed down to the last one — only the track the user actually lands on should be
/// loaded/started. A non-`Play` command pulled ahead while coalescing is handed back via
/// `pending` and returned first on the next call, so nothing skips ahead of it out of order.
fn next_command(rx: &Receiver<PlayerCommand>, pending: &mut Option<PlayerCommand>) -> Option<PlayerCommand> {
    let msg = match pending.take() {
        Some(msg) => msg,
        None => rx.recv().ok()?,
    };
    Some(if matches!(msg, PlayerCommand::Play(..)) {
        let mut latest = msg;
        while let Ok(next) = rx.try_recv() {
            if matches!(next, PlayerCommand::Play(..)) {
                latest = next;
            } else {
                *pending = Some(next);
                break;
            }
        }
        latest
    } else {
        msg
    })
}

pub(crate) fn player_loop(
    rx: Receiver<PlayerCommand>,
    token: Arc<Mutex<Token>>,
    is_playing_flag: Arc<AtomicBool>,
    is_seeking_flag: Arc<AtomicBool>,
    sink_arc: Arc<Mutex<Option<Sink>>>,
    position: Arc<Mutex<Position>>,
    wave_buffer: Arc<Mutex<VecDeque<f32>>>,
) {
    let stream = open_output_stream();
    let mut engine = PlaybackEngine::new(Arc::clone(&stream)).unwrap();

    let mut pending: Option<PlayerCommand> = None;
    while let Some(msg) = next_command(&rx, &mut pending) {
        match msg {
            PlayerCommand::Play(track, change) => {
                engine.play_from_position(
                    &track,
                    0,
                    change,
                    &token,
                    &sink_arc,
                    &is_playing_flag,
                    &position,
                    &wave_buffer,
                );
            }

            PlayerCommand::PreloadNext(track) => {
                let _ = engine.preload_next_track(&track, &token);
            }

            PlayerCommand::SetCrossfade { ms, on_user_skips } => {
                engine.set_crossfade(ms, on_user_skips)
            }

            PlayerCommand::Pause => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.pause();
                    is_playing_flag.store(false, Ordering::SeqCst);

                    let mut pos = position.lock().unwrap();
                    if let Some(start) = pos.last_start {
                        pos.elapsed += start.elapsed();
                    }
                    pos.last_start = None;
                }
            }

            PlayerCommand::Resume => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.play();
                    is_playing_flag.store(true, Ordering::SeqCst);
                    position.lock().unwrap().last_start = Some(Instant::now());
                }
            }

            PlayerCommand::VolumeUp => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    let new_volume = (s.volume() + 0.1).min(2.0);
                    s.set_volume(new_volume);
                }
            }

            PlayerCommand::VolumeDown => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    let new_volume = (s.volume() - 0.1).max(0.0);
                    s.set_volume(new_volume);
                }
            }

            PlayerCommand::FastForward => {
                if is_seeking_flag.swap(true, Ordering::SeqCst) {
                    continue;
                }
                let pos = position.lock().unwrap();
                if let Some(track) = pos.track.clone() {
                    let current_elapsed = if is_playing_flag.load(Ordering::SeqCst) {
                        if let Some(start) = pos.last_start {
                            pos.elapsed + start.elapsed()
                        } else {
                            pos.elapsed
                        }
                    } else {
                        pos.elapsed
                    };

                    let new_elapsed = current_elapsed + Duration::from_secs(10);
                    let max_duration = Duration::from_millis(track.duration_ms);

                    drop(pos);

                    if new_elapsed >= max_duration {
                        // Leave the track in the same state as a natural end-of-track
                        // (is_playing still true, elapsed == duration) so the TUI tick's
                        // end-of-track handling advances to the next track.
                        if let Some(ref s) = *sink_arc.lock().unwrap() {
                            s.stop();
                        }
                        let mut pos = position.lock().unwrap();
                        pos.elapsed = max_duration;
                        pos.last_start = None;
                    } else {
                        let new_position_ms = new_elapsed.as_millis() as u64;
                        engine.play_from_position(
                            &track,
                            new_position_ms,
                            TrackChange::Seek,
                            &token,
                            &sink_arc,
                            &is_playing_flag,
                            &position,
                            &wave_buffer,
                        );
                    }
                }
                is_seeking_flag.store(false, Ordering::SeqCst);
            }

            PlayerCommand::Rewind => {
                if is_seeking_flag.swap(true, Ordering::SeqCst) {
                    continue;
                }
                let pos = position.lock().unwrap();
                if let Some(track) = pos.track.clone() {
                    let current_elapsed = if is_playing_flag.load(Ordering::SeqCst) {
                        if let Some(start) = pos.last_start {
                            pos.elapsed + start.elapsed()
                        } else {
                            pos.elapsed
                        }
                    } else {
                        pos.elapsed
                    };

                    let rewind_duration = Duration::from_secs(10);
                    let new_elapsed = if current_elapsed > rewind_duration {
                        current_elapsed - rewind_duration
                    } else {
                        Duration::ZERO
                    };

                    drop(pos);

                    let new_position_ms = new_elapsed.as_millis() as u64;
                    engine.play_from_position(
                        &track,
                        new_position_ms,
                        TrackChange::Seek,
                        &token,
                        &sink_arc,
                        &is_playing_flag,
                        &position,
                        &wave_buffer,
                    );
                }
                is_seeking_flag.store(false, Ordering::SeqCst);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Track;
    use std::sync::mpsc;

    fn track(urn: &str) -> Track {
        Track {
            title: String::new(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: urn.into(),
        }
    }

    fn urn(cmd: &PlayerCommand) -> &str {
        match cmd {
            PlayerCommand::Play(t, _) => &t.track_urn,
            _ => panic!("expected Play"),
        }
    }

    #[test]
    fn a_burst_of_plays_collapses_to_the_last_one() {
        let (tx, rx) = mpsc::channel();
        tx.send(PlayerCommand::Play(track("a"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Play(track("b"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Play(track("c"), TrackChange::UserSkip)).unwrap();

        let mut pending = None;
        let msg = next_command(&rx, &mut pending).unwrap();
        assert_eq!(urn(&msg), "c");
        assert!(pending.is_none());
        assert!(rx.try_recv().is_err(), "the whole burst should be drained");
    }

    #[test]
    fn a_non_play_command_after_a_burst_is_returned_next_in_order() {
        let (tx, rx) = mpsc::channel();
        tx.send(PlayerCommand::Play(track("a"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Play(track("b"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Pause).unwrap();
        tx.send(PlayerCommand::Play(track("c"), TrackChange::UserSkip)).unwrap();

        let mut pending = None;
        let first = next_command(&rx, &mut pending).unwrap();
        assert_eq!(urn(&first), "b", "a and b collapse; pause stops the drain");
        assert!(matches!(pending, Some(PlayerCommand::Pause)));

        let second = next_command(&rx, &mut pending).unwrap();
        assert!(matches!(second, PlayerCommand::Pause));
        assert!(pending.is_none());

        let third = next_command(&rx, &mut pending).unwrap();
        assert_eq!(urn(&third), "c");
    }

    #[test]
    fn single_play_with_nothing_queued_passes_through_unchanged() {
        let (tx, rx) = mpsc::channel();
        tx.send(PlayerCommand::Play(track("only"), TrackChange::UserSkip)).unwrap();
        let mut pending = None;
        assert_eq!(urn(&next_command(&rx, &mut pending).unwrap()), "only");
    }

    #[test]
    fn closed_channel_ends_the_loop() {
        let (tx, rx) = mpsc::channel::<PlayerCommand>();
        drop(tx);
        let mut pending = None;
        assert!(next_command(&rx, &mut pending).is_none());
    }
}
