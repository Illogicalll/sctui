use anyhow::Context;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::io::Cursor;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use url::Url;

use crate::api::Track;
use crate::auth::{Token, try_refresh_token};
use crate::player::stream::cache::{CachedHls, SegmentCache, SEGMENT_CACHE_CAP};
use crate::player::stream::hls::{HlsManifest, StreamsResponse};
use crate::player::stream::downloader::spawn_segment_pump;
use crate::player::stream::sample::TapSource;

pub(crate) const CROSSFADE_DURATION: Duration = Duration::from_millis(35);
const CROSSFADE_STEPS: usize = 7;

pub(crate) fn open_output_stream() -> Arc<Mutex<OutputStream>> {
    let output_stream = OutputStreamBuilder::open_default_stream().unwrap();
    Arc::new(Mutex::new(output_stream))
}

pub(crate) struct PlaybackEngine {
    stream: Arc<Mutex<OutputStream>>,
    client: reqwest::blocking::Client,
    generation: Arc<AtomicU64>,
    cache: Option<CachedHls>,
    preload_next: Option<CachedHls>,
}

impl PlaybackEngine {
    pub(crate) fn new(stream: Arc<Mutex<OutputStream>>) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("sctui")
            .timeout(Duration::from_secs(15))
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self {
            stream,
            client,
            generation: Arc::new(AtomicU64::new(0)),
            cache: None,
            preload_next: None,
        })
    }

    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn get_hls_url(&self, track_urn: &str, access_token: &str) -> anyhow::Result<Url> {
        let streams_url = format!("https://api.soundcloud.com/tracks/{}/streams", track_urn);
        let streams_response: StreamsResponse = self
            .client
            .get(&streams_url)
            .bearer_auth(access_token)
            .send()
            .context("failed to fetch streams endpoint")?
            .error_for_status()
            .context("streams endpoint returned error status")?
            .json()
            .context("failed to parse streams response json")?;

        let hls_url = streams_response
            .hls_aac_160_url
            .or(streams_response.hls_aac_96_url)
            .or(streams_response.hls_mp3_128_url)
            .ok_or_else(|| anyhow::anyhow!("No HLS stream URL available (tried AAC 160, AAC 96, MP3 128)"))?;

        Url::parse(&hls_url).context("invalid HLS URL")
    }

    fn download_bytes(&self, url: &Url) -> anyhow::Result<Vec<u8>> {
        let bytes = self
            .client
            .get(url.as_str())
            .send()
            .with_context(|| format!("failed to download {}", url))?
            .error_for_status()
            .with_context(|| format!("download returned error status {}", url))?
            .bytes()
            .with_context(|| format!("failed to read bytes {}", url))?;
        Ok(bytes.to_vec())
    }

    fn ensure_cached_hls(
        &mut self,
        track: &Track,
        token: &Arc<Mutex<Token>>,
    ) -> anyhow::Result<(Arc<HlsManifest>, Arc<Vec<u8>>, Arc<Mutex<SegmentCache>>)> {
        let now = Instant::now();
        
        if let Some(ref preload) = self.preload_next {
            if preload.track_urn == track.track_urn {
                let preload = self.preload_next.take().unwrap();
                self.cache = Some(preload);
                let cached = self.cache.as_ref().unwrap();
                return Ok((
                    Arc::clone(&cached.manifest),
                    Arc::clone(&cached.init_bytes),
                    Arc::clone(&cached.segment_cache),
                ));
            }
        }
        
        let cache_valid = self.cache.as_ref().is_some_and(|c| c.is_valid_for(track, now));

        if !cache_valid {
            let _ = try_refresh_token(token);
            let access_token = { token.lock().unwrap().access_token.clone() };

            let playlist_url = self.get_hls_url(&track.track_urn, &access_token)?;
            let manifest = HlsManifest::fetch(&self.client, &playlist_url, &access_token)?;

            let init_bytes = if let Some(init_url) = &manifest.init_url {
                Arc::new(self.download_bytes(init_url)?)
            } else {
                Arc::new(Vec::new())
            };

            self.cache = Some(CachedHls {
                track_urn: track.track_urn.clone(),
                fetched_at: now,
                manifest: Arc::new(manifest),
                init_bytes,
                segment_cache: Arc::new(Mutex::new(SegmentCache::new(SEGMENT_CACHE_CAP))),
            });
        }

        let cached = self
            .cache
            .as_ref()
            .expect("cache must be set by now");
        Ok((
            Arc::clone(&cached.manifest),
            Arc::clone(&cached.init_bytes),
            Arc::clone(&cached.segment_cache),
        ))
    }

    pub(crate) fn preload_next_track(
        &mut self,
        track: &Track,
        token: &Arc<Mutex<Token>>,
    ) -> anyhow::Result<()> {
        if self.preload_next.as_ref().is_some_and(|p| p.track_urn == track.track_urn) {
            return Ok(());
        }

        let _ = try_refresh_token(token);
        let access_token = { token.lock().unwrap().access_token.clone() };

        let playlist_url = self.get_hls_url(&track.track_urn, &access_token)?;
        let manifest = HlsManifest::fetch(&self.client, &playlist_url, &access_token)?;

        let init_bytes = if let Some(init_url) = &manifest.init_url {
            Arc::new(self.download_bytes(init_url)?)
        } else {
            Arc::new(Vec::new())
        };

        let first_segment_bytes = if !manifest.segments.is_empty() {
            match self.download_bytes(&manifest.segments[0].url) {
                Ok(bytes) => {
                    let mut cache = SegmentCache::new(SEGMENT_CACHE_CAP);
                    cache.insert(0, Arc::new(bytes));
                    Some(Arc::new(Mutex::new(cache)))
                }
                Err(_) => {
                    None
                }
            }
        } else {
            None
        };

        self.preload_next = Some(CachedHls {
            track_urn: track.track_urn.clone(),
            fetched_at: Instant::now(),
            manifest: Arc::new(manifest),
            init_bytes,
            segment_cache: first_segment_bytes.unwrap_or_else(|| {
                Arc::new(Mutex::new(SegmentCache::new(SEGMENT_CACHE_CAP)))
            }),
        });

        Ok(())
    }

    pub(crate) fn play_from_position(
        &mut self,
        track: &Track,
        position_ms: u64,
        token: &Arc<Mutex<Token>>,
        sink_arc: &Arc<Mutex<Option<Sink>>>,
        is_playing_flag: &Arc<std::sync::atomic::AtomicBool>,
        elapsed_time: &Arc<Mutex<Duration>>,
        last_start: &Arc<Mutex<Option<Instant>>>,
        current_track: &Arc<Mutex<Option<Track>>>,
        wave_buffer: &Arc<Mutex<std::collections::VecDeque<f32>>>,
    ) {
        let old_track_urn = current_track
            .lock()
            .unwrap()
            .as_ref()
            .map(|t| t.track_urn.clone());
        let is_seek = old_track_urn.as_deref() == Some(&track.track_urn);

        let (has_old_sink, target_volume) = {
            let guard = sink_arc.lock().unwrap();
            let vol = guard.as_ref().map(|s| s.volume()).unwrap_or(1.0);
            (guard.is_some(), vol)
        };

        let planned_generation = self.current_generation().wrapping_add(1);
        if !is_seek {
            let generation_id = self.bump_generation();
            debug_assert_eq!(generation_id, planned_generation);
            if let Some(ref s) = *sink_arc.lock().unwrap() {
                s.stop();
            }
        }

        let (manifest, init_bytes, segment_cache) = match self.ensure_cached_hls(track, token) {
            Ok(v) => v,
            Err(_) => {
                if !is_seek {
                    is_playing_flag.store(false, Ordering::SeqCst);
                    *last_start.lock().unwrap() = None;
                }
                return;
            }
        };

        let (segment_index, offset_within_segment_ms) = manifest.locate_position(position_ms);

        let media_bytes = {
            let mut cache_guard = segment_cache.lock().unwrap();
            if let Some(bytes) = cache_guard.get(segment_index) {
                bytes
            } else {
                drop(cache_guard);
                match self.download_bytes(&manifest.segments[segment_index].url) {
                    Ok(bytes) => {
                        let arc = Arc::new(bytes);
                        let mut cache_guard = segment_cache.lock().unwrap();
                        cache_guard.insert(segment_index, Arc::clone(&arc));
                        arc
                    }
                    Err(_) => {
                        if !is_seek {
                            is_playing_flag.store(false, Ordering::SeqCst);
                            *last_start.lock().unwrap() = None;
                        }
                        return;
                    }
                }
            }
        };

        let first_bytes = combine_init_and_segment(&init_bytes, &media_bytes);

        let new_sink = {
            let stream_guard = self.stream.lock().unwrap();
            Sink::connect_new(stream_guard.mixer())
        };

        new_sink.set_volume(target_volume);

        if append_segment_to_sink(
            &new_sink,
            first_bytes,
            wave_buffer,
            offset_within_segment_ms,
        ).is_err() {
            if !is_seek {
                is_playing_flag.store(false, Ordering::SeqCst);
                *last_start.lock().unwrap() = None;
            }
            return;
        }

        let gen_for_pump = if is_seek {
            let generation_id = self.bump_generation();
            debug_assert_eq!(generation_id, planned_generation);
            generation_id
        } else {
            planned_generation
        };

        let old_sink_for_fade = if is_seek && has_old_sink {
            sink_arc.lock().unwrap().take()
        } else {
            None
        };

        *sink_arc.lock().unwrap() = Some(new_sink);

        if let Some(old_sink) = old_sink_for_fade {
            crossfade_and_stop(old_sink, target_volume);
        }

        *current_track.lock().unwrap() = Some(track.clone());
        *elapsed_time.lock().unwrap() = Duration::from_millis(position_ms);
        *last_start.lock().unwrap() = Some(Instant::now());
        is_playing_flag.store(true, Ordering::SeqCst);

        use crate::player::stream::downloader::SegmentPumpParams;
        spawn_segment_pump(SegmentPumpParams {
            client: self.client.clone(),
            generation: Arc::clone(&self.generation),
            generation_value: gen_for_pump,
            manifest,
            init_bytes,
            segment_cache,
            start_segment_index: segment_index,
            sink_arc: Arc::clone(sink_arc),
            wave_buffer: Arc::clone(wave_buffer),
            is_playing_flag: Arc::clone(is_playing_flag),
            elapsed_time: Arc::clone(elapsed_time),
            last_start: Arc::clone(last_start),
        });
    }
}

fn combine_init_and_segment(init_bytes: &[u8], segment_bytes: &[u8]) -> Vec<u8> {
    let mut combined = Vec::with_capacity(init_bytes.len() + segment_bytes.len());
    combined.extend_from_slice(init_bytes);
    combined.extend_from_slice(segment_bytes);
    combined
}

fn append_segment_to_sink(
    sink: &Sink,
    bytes: Vec<u8>,
    wave_buffer: &Arc<Mutex<std::collections::VecDeque<f32>>>,
    skip_ms: u64,
) -> anyhow::Result<()> {
    let cursor = Cursor::new(bytes);
    let decoder = Decoder::new(cursor).context("rodio decoder init failed")?;

    if skip_ms > 0 {
        let skipped = decoder.skip_duration(Duration::from_millis(skip_ms));
        let tapped = TapSource::new(skipped, Arc::clone(wave_buffer));
        sink.append(tapped);
    } else {
        let tapped = TapSource::new(decoder, Arc::clone(wave_buffer));
        sink.append(tapped);
    }

    Ok(())
}

fn crossfade_and_stop(old_sink: Sink, target_volume: f32) {
    let steps = CROSSFADE_STEPS.max(1);
    let total_ms = CROSSFADE_DURATION.as_millis().max(1) as u64;
    let step_ms = (total_ms / steps as u64).max(1);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        old_sink.set_volume(target_volume * (1.0 - t));
        std::thread::sleep(Duration::from_millis(step_ms));
    }

    old_sink.stop();
}
