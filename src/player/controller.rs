use crate::api::Track;
use crate::auth::Token;
use rodio::Sink;
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

use super::commands::PlayerCommand;
use super::worker::player_loop;

pub struct Player {
    tx: Sender<PlayerCommand>,
    is_playing_flag: Arc<AtomicBool>,
    is_seeking_flag: Arc<AtomicBool>,
    elapsed_time: Arc<Mutex<Duration>>,
    last_start: Arc<Mutex<Option<Instant>>>,
    current_track: Arc<Mutex<Option<Track>>>,
    sink: Arc<Mutex<Option<Sink>>>,
    wave_buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl Player {
    pub fn new(token: Arc<Mutex<Token>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let is_playing_flag = Arc::new(AtomicBool::new(false));
        let is_seeking_flag = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(Mutex::new(None));
        let elapsed_time = Arc::new(Mutex::new(Duration::ZERO));
        let last_start = Arc::new(Mutex::new(None));
        let current_track = Arc::new(Mutex::new(None));
        let wave_buffer = Arc::new(Mutex::new(VecDeque::new()));

        {
            let flag_clone = Arc::clone(&is_playing_flag);
            let sink_clone = Arc::clone(&sink);
            let token_clone = Arc::clone(&token);
            let elapsed_clone = Arc::clone(&elapsed_time);
            let last_start_clone = Arc::clone(&last_start);
            let track_clone = Arc::clone(&current_track);
            let seeking_clone = Arc::clone(&is_seeking_flag);
            let wave_buffer_clone = Arc::clone(&wave_buffer);

            thread::spawn(move || {
                player_loop(
                    rx,
                    token_clone,
                    flag_clone,
                    seeking_clone,
                    sink_clone,
                    elapsed_clone,
                    last_start_clone,
                    track_clone,
                    wave_buffer_clone,
                );
            });
        }

        Self {
            tx,
            is_playing_flag,
            is_seeking_flag,
            elapsed_time,
            last_start,
            current_track,
            sink,
            wave_buffer,
        }
    }

    pub fn play(&self, track: Track) {
        let _ = self.tx.send(PlayerCommand::Play(track));
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

    pub fn next_song(&self) {
        let _ = self.tx.send(PlayerCommand::NextSong);
    }

    pub fn prev_song(&self) {
        let _ = self.tx.send(PlayerCommand::PrevSong);
    }

    pub fn fast_forward(&self) {
        let _ = self.tx.send(PlayerCommand::FastForward);
    }

    pub fn rewind(&self) {
        let _ = self.tx.send(PlayerCommand::Rewind);
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing_flag.load(Ordering::SeqCst)
    }

    pub fn is_seeking(&self) -> bool {
        self.is_seeking_flag.load(Ordering::SeqCst)
    }

    pub fn elapsed(&self) -> u64 {
        let mut elapsed = *self.elapsed_time.lock().unwrap();
        if self.is_playing() {
            if let Some(start) = *self.last_start.lock().unwrap() {
                elapsed += start.elapsed();
            }
        }
        elapsed.as_millis().try_into().unwrap()
    }

    pub fn current_track(&self) -> Track {
        self.current_track
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| Track {
                title: "No Track Playing - Press <ENTER> on Something to Play!".to_string(),
                artists: "N/A".to_string(),
                duration: "0:00".to_string(),
                duration_ms: 1,
                playback_count: "0".to_string(),
                artwork_url: "".to_string(),
                stream_url: "".to_string(),
                access: "playable".to_string(),
            })
    }

    pub fn get_volume(&self) -> f32 {
        if let Some(ref s) = *self.sink.lock().unwrap() {
            s.volume()
        } else {
            1.0
        }
    }

    pub fn wave_buffer(&self) -> Arc<Mutex<VecDeque<f32>>> {
        Arc::clone(&self.wave_buffer)
    }
}
