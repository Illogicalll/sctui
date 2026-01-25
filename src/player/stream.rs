use crate::api::Track;
use crate::auth::{Token, try_refresh_token};
use dasp_sample::ToSample;
use reqwest::header::{HeaderMap, HeaderValue};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

const WAVE_BUFFER_CAP: usize = 2048;

struct TapSource<S> {
    inner: S,
    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl<S> TapSource<S> {
    fn new(inner: S, buffer: Arc<Mutex<VecDeque<f32>>>) -> Self {
        Self { inner, buffer }
    }
}

impl<S> Iterator for TapSource<S>
where
    S: Source,
    S::Item: ToSample<f32>,
{
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let sample_f32: f32 = sample.to_sample_();
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.len() >= WAVE_BUFFER_CAP {
            let overflow = buffer.len() + 1 - WAVE_BUFFER_CAP;
            for _ in 0..overflow {
                buffer.pop_front();
            }
        }
        buffer.push_back(sample_f32);
        Some(sample)
    }
}

impl<S> Source for TapSource<S>
where
    S: Source,
    S::Item: ToSample<f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

pub(crate) fn setup_stream() -> (tokio::runtime::Runtime, Arc<Mutex<OutputStream>>) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let output_stream = OutputStreamBuilder::open_default_stream().unwrap();
    let stream = Arc::new(Mutex::new(output_stream));

    (rt, stream)
}

pub(crate) fn play_from_position(
    track: &Track,
    position_ms: u64,
    token: &Arc<Mutex<Token>>,
    sink_arc: &Arc<Mutex<Option<Sink>>>,
    is_playing_flag: &Arc<std::sync::atomic::AtomicBool>,
    elapsed_time: &Arc<Mutex<Duration>>,
    last_start: &Arc<Mutex<Option<Instant>>>,
    current_track: &Arc<Mutex<Option<Track>>>,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
    stream: &Arc<Mutex<OutputStream>>,
    rt: &tokio::runtime::Runtime,
) {
    if let Some(ref s) = *sink_arc.lock().unwrap() {
        s.stop();
    }

    is_playing_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    *elapsed_time.lock().unwrap() = Duration::from_millis(position_ms);
    *last_start.lock().unwrap() = Some(Instant::now());
    *current_track.lock().unwrap() = Some(track.clone());

    let token_clone = Arc::clone(token);
    let sink_store = Arc::clone(sink_arc);
    let flag_clone = Arc::clone(is_playing_flag);
    let stream_clone = Arc::clone(stream);
    let wave_buffer_clone = Arc::clone(wave_buffer);

    let stream_url = track.stream_url.clone();
    let seek_position = position_ms;

    rt.block_on(async move {
        let _ = try_refresh_token(&token_clone);

        let token_guard = token_clone.lock().unwrap();
        let mut headers = HeaderMap::new();
        let mut header: HeaderValue = format!("OAuth {}", token_guard.access_token)
            .parse()
            .unwrap();
        header.set_sensitive(true);
        headers.insert("Authorization", header);

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
            Ok(stream) => match StreamDownload::from_stream(
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
                    let decoder = Decoder::new(reader).unwrap();
                    let tapped = TapSource::new(decoder, wave_buffer_clone);
                    sink.append(tapped);
                    *sink_store.lock().unwrap() = Some(sink);
                }
                Err(_) => {
                    flag_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                }
            },
            Err(_) => {
                flag_clone.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
    });
}
