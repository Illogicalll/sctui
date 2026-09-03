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
use super::commands::PlayerCommand;
use super::stream::{PlaybackEngine, open_output_stream};

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

    for msg in rx {
        match msg {
            PlayerCommand::Play(track) => {
                engine.play_from_position(
                    &track,
                    0,
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
                        if let Some(ref s) = *sink_arc.lock().unwrap() {
                            s.stop();
                        }
                        is_playing_flag.store(false, Ordering::SeqCst);
                        let mut pos = position.lock().unwrap();
                        pos.elapsed = max_duration;
                        pos.last_start = None;
                    } else {
                        let new_position_ms = new_elapsed.as_millis() as u64;
                        engine.play_from_position(
                            &track,
                            new_position_ms,
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
