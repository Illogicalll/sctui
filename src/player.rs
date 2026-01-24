use crate::api::Track;
use crate::auth::Token;
use reqwest::header::{HeaderMap, HeaderValue};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::thread;
use std::time::{Duration, Instant};
use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

pub enum PlayerCommand {
    Play(Track),
    PlayFromPosition(Track, u64),
    Pause,
    Resume,
    VolumeUp,
    VolumeDown,
    NextSong,
    PrevSong,
    FastForward,
    Rewind,
}

pub struct Player {
    tx: Sender<PlayerCommand>,
    is_playing_flag: Arc<AtomicBool>,
    elapsed_time: Arc<Mutex<Duration>>,
    last_start: Arc<Mutex<Option<Instant>>>,
    current_track: Arc<Mutex<Option<Track>>>,
    sink: Arc<Mutex<Option<Sink>>>,
}

impl Player {
    pub fn new(token: Arc<Mutex<Token>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let is_playing_flag = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(Mutex::new(None));
        let elapsed_time = Arc::new(Mutex::new(Duration::ZERO));
        let last_start = Arc::new(Mutex::new(None));
        let current_track = Arc::new(Mutex::new(None));

        {
            let flag_clone = Arc::clone(&is_playing_flag);
            let sink_clone = Arc::clone(&sink);
            let token_clone = Arc::clone(&token);
            let elapsed_clone = Arc::clone(&elapsed_time);
            let last_start_clone = Arc::clone(&last_start);
            let track_clone = Arc::clone(&current_track);

            thread::spawn(move || {
                player_loop(
                    rx,
                    token_clone,
                    flag_clone,
                    sink_clone,
                    elapsed_clone,
                    last_start_clone,
                    track_clone,
                );
            });
        }

        Self {
            tx,
            is_playing_flag,
            elapsed_time,
            last_start,
            current_track,
            sink,
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
            })
    }

    pub fn get_volume(&self) -> f32 {
        if let Some(ref s) = *self.sink.lock().unwrap() {
            s.volume()
        } else {
            1.0
        }
    }
}

// helper function to play a track from a specific position (in milliseconds)
fn play_from_position(
    track: &Track,
    position_ms: u64,
    token: &Arc<Mutex<Token>>,
    sink_arc: &Arc<Mutex<Option<Sink>>>,
    is_playing_flag: &Arc<AtomicBool>,
    elapsed_time: &Arc<Mutex<Duration>>,
    last_start: &Arc<Mutex<Option<Instant>>>,
    current_track: &Arc<Mutex<Option<Track>>>,
    stream: &Arc<Mutex<OutputStream>>,
    rt: &tokio::runtime::Runtime,
) {
        if let Some(ref s) = *sink_arc.lock().unwrap() {
            s.stop();
        }

        is_playing_flag.store(true, Ordering::SeqCst);
        *elapsed_time.lock().unwrap() = Duration::from_millis(position_ms);
        *last_start.lock().unwrap() = Some(Instant::now());
        *current_track.lock().unwrap() = Some(track.clone());

        let token_clone = Arc::clone(token);
        let sink_store = Arc::clone(sink_arc);
        let flag_clone = Arc::clone(is_playing_flag);
        let stream_clone = Arc::clone(stream);

        let stream_url = track.stream_url.clone();
        let seek_position = position_ms;

        rt.block_on(async move {
            let token_guard = token_clone.lock().unwrap();
            let mut headers = HeaderMap::new();
            let mut header: HeaderValue = format!("OAuth {}", token_guard.access_token)
                .parse()
                .unwrap();
            header.set_sensitive(true);
            headers.insert("Authorization", header);
            

            // estimate byte position: assume ~128 kbps = ~16 KB per second
            if seek_position > 0 {
                let estimated_bytes_per_second = 16_000;
                let start_byte = (seek_position as u64 / 1000) * estimated_bytes_per_second;
                let range_header = format!("bytes={}-", start_byte);
                if let Ok(range_value) = HeaderValue::from_str(&range_header) {
                    headers.insert("Range", range_value);
                }
            }
            
            drop(token_guard);

            let client = Client::builder().default_headers(headers).build().unwrap();

            match HttpStream::new(client, stream_url.parse().unwrap()).await {
                Ok(stream) => {
                    match StreamDownload::from_stream(
                        stream,
                        TempStorageProvider::new(),
                        Settings::default(),
                    )
                    .await
                    {
                        Ok(reader) => {
                            let sink = {
                                let stream_guard = stream_clone.lock().unwrap();
                                Sink::connect_new(stream_guard.mixer())
                            };
                            sink.append(Decoder::new(reader).unwrap());
                            *sink_store.lock().unwrap() = Some(sink);
                        }
                        Err(_) => {
                            flag_clone.store(false, Ordering::SeqCst);
                        }
                    }
                }
                Err(_) => {
                    flag_clone.store(false, Ordering::SeqCst);
                }
            }
        });
}

fn player_loop(
    rx: Receiver<PlayerCommand>,
    token: Arc<Mutex<Token>>,
    is_playing_flag: Arc<AtomicBool>,
    sink_arc: Arc<Mutex<Option<Sink>>>,
    elapsed_time: Arc<Mutex<Duration>>,
    last_start: Arc<Mutex<Option<Instant>>>,
    current_track: Arc<Mutex<Option<Track>>>,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let output_stream = OutputStreamBuilder::open_default_stream().unwrap();
    let stream = Arc::new(Mutex::new(output_stream));

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
                let current_track_guard = current_track.lock().unwrap();
                if let Some(track) = current_track_guard.clone() {
                    drop(current_track_guard);
                    
                    let mut elapsed = elapsed_time.lock().unwrap();
                    let current_elapsed = if is_playing_flag.load(Ordering::SeqCst) {
                        if let Some(start) = *last_start.lock().unwrap() {
                            *elapsed + start.elapsed()
                        } else {
                            *elapsed
                        }
                    } else {
                        *elapsed
                    };
                    
                    // fast forward by 10 seconds (10000 milliseconds)
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
                        // restart playback from new position
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
            }

            PlayerCommand::Rewind => {
                let current_track_guard = current_track.lock().unwrap();
                if let Some(track) = current_track_guard.clone() {
                    drop(current_track_guard);
                    
                    let mut elapsed = elapsed_time.lock().unwrap();
                    let current_elapsed = if is_playing_flag.load(Ordering::SeqCst) {
                        if let Some(start) = *last_start.lock().unwrap() {
                            *elapsed + start.elapsed()
                        } else {
                            *elapsed
                        }
                    } else {
                        *elapsed
                    };
                    
                    // rewind by 10 seconds (10000 milliseconds)
                    let rewind_duration = Duration::from_secs(10);
                    let new_elapsed = if current_elapsed > rewind_duration {
                        current_elapsed - rewind_duration
                    } else {
                        Duration::ZERO
                    };
                    
                    drop(elapsed);
                    
                    // restart playback from new position
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
        }
    }
}
