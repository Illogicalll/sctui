use crate::auth::Token;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};
use rodio::{Decoder, OutputStreamBuilder, Sink};
use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

pub enum PlayerCommand {
    Play(String),
    Pause,
    Resume,
}

pub struct Player {
    tx: Sender<PlayerCommand>,
    is_playing_flag: Arc<AtomicBool>,
    start_instant: Arc<Mutex<Option<Instant>>>,
    paused_instant: Arc<Mutex<Option<Instant>>>,
    paused_duration: Arc<Mutex<Duration>>,
}

impl Player {
    pub fn new(token: Arc<Mutex<Token>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let is_playing_flag = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(Mutex::new(None));
        let start_instant = Arc::new(Mutex::new(None));
        let paused_instant = Arc::new(Mutex::new(None));
        let paused_duration = Arc::new(Mutex::new(Duration::ZERO));

        // spawn player thread
        {
            let flag_clone = Arc::clone(&is_playing_flag);
            let sink_clone = Arc::clone(&sink);
            let start_clone = Arc::clone(&start_instant);
            let paused_clone = Arc::clone(&paused_instant);
            let paused_dur_clone = Arc::clone(&paused_duration);
            let token_clone = Arc::clone(&token);

            thread::spawn(move || {
                player_loop(
                    rx,
                    token_clone,
                    flag_clone,
                    sink_clone,
                    start_clone,
                    paused_clone,
                    paused_dur_clone,
                );
            });
        }

        Self {
            tx,
            is_playing_flag,
            start_instant,
            paused_instant,
            paused_duration,
        }
    }

    pub fn play(&self, url: String) {
        let _ = self.tx.send(PlayerCommand::Play(url));
    }

    pub fn pause(&self) {
        let _ = self.tx.send(PlayerCommand::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(PlayerCommand::Resume);
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing_flag.load(Ordering::SeqCst)
    }

    pub fn elapsed(&self) -> u64 {
        let start_opt = *self.start_instant.lock().unwrap();
        if let Some(start) = start_opt {
            let paused_dur = *self.paused_duration.lock().unwrap();
            let paused_since = *self.paused_instant.lock().unwrap();
            let extra_pause = paused_since.map_or(Duration::ZERO, |p| Instant::now() - p);
            let total_elapsed = Instant::now() - start - (paused_dur + extra_pause);
            total_elapsed.as_millis() as u64
        } else {
            0
        }
    }
}

fn player_loop(
    rx: Receiver<PlayerCommand>,
    token: Arc<Mutex<Token>>,
    is_playing_flag: Arc<AtomicBool>,
    sink_arc: Arc<Mutex<Option<Sink>>>,
    start_instant: Arc<Mutex<Option<Instant>>>,
    paused_instant: Arc<Mutex<Option<Instant>>>,
    paused_duration: Arc<Mutex<Duration>>,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // keep the OutputStream alive for the entire duration of the player loop
    let stream = Arc::new(Mutex::new(
        OutputStreamBuilder::open_default_stream().unwrap(),
    ));

    for msg in rx {
        match msg {
            PlayerCommand::Play(url) => {
                // stop any current playback
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.stop();
                }

                // reset timing info
                *paused_instant.lock().unwrap() = None;
                *paused_duration.lock().unwrap() = Duration::ZERO;
                *start_instant.lock().unwrap() = Some(Instant::now());
                is_playing_flag.store(true, Ordering::SeqCst);

                let token_clone = Arc::clone(&token);
                let sink_store = Arc::clone(&sink_arc);
                let flag_clone = Arc::clone(&is_playing_flag);
                let stream_clone = Arc::clone(&stream);

                rt.block_on(async move {
                    let token_guard = token_clone.lock().unwrap();
                    let mut headers = HeaderMap::new();
                    let mut header: HeaderValue = format!("OAuth {}", token_guard.access_token)
                        .parse()
                        .unwrap();
                    header.set_sensitive(true);
                    headers.insert("Authorization", header);
                    drop(token_guard);

                    let client = Client::builder().default_headers(headers).build().unwrap();

                    match HttpStream::new(client, url.parse().unwrap()).await {
                        Ok(stream) => {
                            match StreamDownload::from_stream(
                                stream,
                                TempStorageProvider::new(),
                                Settings::default(),
                            )
                            .await
                            {
                                Ok(reader) => {
                                    // create the sink and immediately drop the stream guard
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

            PlayerCommand::Pause => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.pause();
                    *paused_instant.lock().unwrap() = Some(Instant::now());
                    is_playing_flag.store(false, Ordering::SeqCst);
                }
            }

            // TODO: fix resume messing with thread
            PlayerCommand::Resume => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.play();
                    if let Some(pause_start) = *paused_instant.lock().unwrap() {
                        *paused_duration.lock().unwrap() += Instant::now() - pause_start;
                        *paused_instant.lock().unwrap() = None;
                    }
                    is_playing_flag.store(true, Ordering::SeqCst);
                }
            }
        }
    }
}
