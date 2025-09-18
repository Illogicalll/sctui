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
    elapsed_time: Arc<Mutex<Duration>>,
    last_start: Arc<Mutex<Option<Instant>>>,
}

impl Player {
    pub fn new(token: Arc<Mutex<Token>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let is_playing_flag = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(Mutex::new(None));
        let elapsed_time = Arc::new(Mutex::new(Duration::ZERO));
        let last_start = Arc::new(Mutex::new(None));

        {
            let flag_clone = Arc::clone(&is_playing_flag);
            let sink_clone = Arc::clone(&sink);
            let token_clone = Arc::clone(&token);
            let elapsed_clone = Arc::clone(&elapsed_time);
            let last_start_clone = Arc::clone(&last_start);

            thread::spawn(move || {
                player_loop(
                    rx,
                    token_clone,
                    flag_clone,
                    sink_clone,
                    elapsed_clone,
                    last_start_clone,
                );
            });
        }

        Self {
            tx,
            is_playing_flag,
            elapsed_time,
            last_start,
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
        let mut elapsed = *self.elapsed_time.lock().unwrap();
        if self.is_playing() {
            if let Some(start) = *self.last_start.lock().unwrap() {
                elapsed += start.elapsed();
            }
        }
        elapsed.as_millis().try_into().unwrap()
    }
}

fn player_loop(
    rx: Receiver<PlayerCommand>,
    token: Arc<Mutex<Token>>,
    is_playing_flag: Arc<AtomicBool>,
    sink_arc: Arc<Mutex<Option<Sink>>>,
    elapsed_time: Arc<Mutex<Duration>>,
    last_start: Arc<Mutex<Option<Instant>>>,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // keep the OutputStream alive for the entire duration of the player loop
    let stream = Arc::new(Mutex::new(
        OutputStreamBuilder::open_default_stream().unwrap(),
    ));

    for msg in rx {
        match msg {
            PlayerCommand::Play(url) => {
                if let Some(ref s) = *sink_arc.lock().unwrap() {
                    s.stop();
                }

                is_playing_flag.store(true, Ordering::SeqCst);
                *elapsed_time.lock().unwrap() = Duration::ZERO;
                *last_start.lock().unwrap() = Some(Instant::now());

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
        }
    }
}

