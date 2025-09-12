use crate::auth::Token;
use std::error::Error;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use reqwest::header::{HeaderMap, HeaderValue};
use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::source::DecodeError;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

// types of commands the player can receive
pub enum PlayerCommand {
    Play(String), // passing in stream_url
}

// define a transmitter that can communicate with the player thread
pub struct Player {
    tx: Sender<PlayerCommand>,
}

impl Player {
    // spawn the player thread
    pub fn new(token: Arc<Mutex<Token>>) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || player_loop(rx, token));
        Player { tx }
    }

    // transmit the stream_url to the player
    pub fn play(&self, url: String) {
        let _ = self.tx.send(PlayerCommand::Play(url));
    }
}

// the player logic that runs in the thread and awaits commands
fn player_loop(rx: Receiver<PlayerCommand>, token: Arc<Mutex<Token>>) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    for msg in rx {
        match msg {
            // play command
            PlayerCommand::Play(url) => {
                let token = token.clone();
                rt.block_on(async move {
                    // build custom HTTP client with Authorization header
                    let token_guard = token.lock().unwrap();
                    let mut headers = HeaderMap::new();
                    let mut header: HeaderValue = format!("OAuth {}", token_guard.access_token)
                        .parse()
                        .unwrap();
                    header.set_sensitive(true);
                    headers.insert("Authorization", header);

                    let client = Client::builder().default_headers(headers).build().unwrap();

                    let stream = match HttpStream::new(client, url.parse().unwrap()).await {
                        Ok(stream) => stream,
                        Err(e) => {
                            eprintln!("Stream error: {}", e.decode_error().await);
                            return;
                        }
                    };

                    drop(token_guard);

                    // download track response
                    let reader = match StreamDownload::from_stream(
                        stream,
                        TempStorageProvider::new(),
                        Settings::default(),
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Download error: {}", e.decode_error().await);
                            return;
                        }
                    };

                    // play audio
                    let handle = tokio::task::spawn_blocking(move || {
                        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
                        let sink = rodio::Sink::connect_new(stream_handle.mixer());
                        sink.append(rodio::Decoder::new(reader)?);
                        sink.sleep_until_end();
                        Ok::<_, Box<dyn Error + Send + Sync>>(())
                    });

                    if let Err(e) = handle.await {
                        eprintln!("Playback task error: {e}");
                    }
                });
            }
        }
    }
}
