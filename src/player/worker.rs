use crate::api::Track;
use crate::auth::Token;
use rodio::Sink;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::Receiver,
};
use std::time::{Duration, Instant};

use super::commands::PlayerCommand;
use super::stream::{play_from_position, setup_stream};

pub(crate) fn player_loop(
    rx: Receiver<PlayerCommand>,
    token: Arc<Mutex<Token>>,
    is_playing_flag: Arc<AtomicBool>,
    is_seeking_flag: Arc<AtomicBool>,
    sink_arc: Arc<Mutex<Option<Sink>>>,
    elapsed_time: Arc<Mutex<Duration>>,
    last_start: Arc<Mutex<Option<Instant>>>,
    current_track: Arc<Mutex<Option<Track>>>,
) {
    let (rt, stream) = setup_stream();

    for msg in rx {
        match msg {
            PlayerCommand::Play(track) => {
                play_from_position(
                    &track,
                    0,
                    &token,
                    &sink_arc,
                    &is_playing_flag,
                    &elapsed_time,
                    &last_start,
                    &current_track,
                    &stream,
                    &rt,
                );
            }

            PlayerCommand::PlayFromPosition(track, position_ms) => {
                play_from_position(
                    &track,
                    position_ms,
                    &token,
                    &sink_arc,
                    &is_playing_flag,
                    &elapsed_time,
                    &last_start,
                    &current_track,
                    &stream,
                    &rt,
                );
            }

            PlayerCommand::Pause => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.pause();
                    is_playing_flag.store(false, Ordering::SeqCst);

                    if let Some(start) = *last_start.lock().unwrap() {
                        let mut elapsed = elapsed_time.lock().unwrap();
                        *elapsed += start.elapsed();
                    }
                    *last_start.lock().unwrap() = None;
                }
            }

            PlayerCommand::Resume => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.play();
                    is_playing_flag.store(true, Ordering::SeqCst);
                    *last_start.lock().unwrap() = Some(Instant::now());
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

            PlayerCommand::NextSong => {}

            PlayerCommand::PrevSong => {}

            PlayerCommand::FastForward => {
                if is_seeking_flag.swap(true, Ordering::SeqCst) {
                    continue;
                }
                let current_track_guard = current_track.lock().unwrap();
                if let Some(track) = current_track_guard.clone() {
                    drop(current_track_guard);

                    let elapsed = elapsed_time.lock().unwrap();
                    let current_elapsed = if is_playing_flag.load(Ordering::SeqCst) {
                        if let Some(start) = *last_start.lock().unwrap() {
                            *elapsed + start.elapsed()
                        } else {
                            *elapsed
                        }
                    } else {
                        *elapsed
                    };

                    let new_elapsed = current_elapsed + Duration::from_secs(10);
                    let max_duration = Duration::from_millis(track.duration_ms);

                    drop(elapsed);

                    if new_elapsed >= max_duration {
                        if let Some(ref s) = *sink_arc.lock().unwrap() {
                            s.stop();
                        }
                        is_playing_flag.store(false, Ordering::SeqCst);
                        *elapsed_time.lock().unwrap() = max_duration;
                        *last_start.lock().unwrap() = None;
                    } else {
                        let new_position_ms = new_elapsed.as_millis() as u64;
                        play_from_position(
                            &track,
                            new_position_ms,
                            &token,
                            &sink_arc,
                            &is_playing_flag,
                            &elapsed_time,
                            &last_start,
                            &current_track,
                            &stream,
                            &rt,
                        );
                    }
                }
                is_seeking_flag.store(false, Ordering::SeqCst);
            }

            PlayerCommand::Rewind => {
                if is_seeking_flag.swap(true, Ordering::SeqCst) {
                    continue;
                }
                let current_track_guard = current_track.lock().unwrap();
                if let Some(track) = current_track_guard.clone() {
                    drop(current_track_guard);

                    let elapsed = elapsed_time.lock().unwrap();
                    let current_elapsed = if is_playing_flag.load(Ordering::SeqCst) {
                        if let Some(start) = *last_start.lock().unwrap() {
                            *elapsed + start.elapsed()
                        } else {
                            *elapsed
                        }
                    } else {
                        *elapsed
                    };

                    let rewind_duration = Duration::from_secs(10);
                    let new_elapsed = if current_elapsed > rewind_duration {
                        current_elapsed - rewind_duration
                    } else {
                        Duration::ZERO
                    };

                    drop(elapsed);

                    let new_position_ms = new_elapsed.as_millis() as u64;
                    play_from_position(
                        &track,
                        new_position_ms,
                        &token,
                        &sink_arc,
                        &is_playing_flag,
                        &elapsed_time,
                        &last_start,
                        &current_track,
                        &stream,
                        &rt,
                    );
                }
                is_seeking_flag.store(false, Ordering::SeqCst);
            }
        }
    }
}
