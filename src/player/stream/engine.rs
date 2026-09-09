use anyhow::Context;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::sync::mpsc::SyncSender;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use reqwest::Url;

use crate::api::Track;
use crate::auth::Token;
use crate::player::Position;
use crate::player::stream::cache::{CachedHls, SegmentCache};
use crate::player::stream::hls::{HlsManifest, resolve_manifest};
use crate::player::stream::downloader::spawn_segment_pump;
use crate::player::stream::eq::EqSource;
use crate::player::stream::reader::{PcmSource, SegmentReader};
use crate::player::stream::sample::TapSource;

pub(crate) const CROSSFADE_DURATION: Duration = Duration::from_millis(35);
const CROSSFADE_STEPS: usize = 7;
/// Decoded audio handed to the output in chunks of this many samples; the
/// channel holds PCM_CHUNKS of them (~1.5 s of stereo 44.1 kHz) as look-ahead.
const PCM_CHUNK_SAMPLES: usize = 2048;
const PCM_CHUNKS: usize = 64;

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
        
        if let Some(ref preload) = self.preload_next
            && preload.track_urn == track.track_urn {
                let preload = self.preload_next.take().unwrap();
                self.cache = Some(preload);
                let cached = self.cache.as_ref().unwrap();
                return Ok((
                    Arc::clone(&cached.manifest),
                    Arc::clone(&cached.init_bytes),
                    Arc::clone(&cached.segment_cache),
                ));
            }
        
        let cache_valid = self.cache.as_ref().is_some_and(|c| c.is_valid_for(track, now));

        if !cache_valid {
            let manifest = resolve_manifest(&self.client, &track.track_urn, token)?;

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
                segment_cache: Arc::new(Mutex::new(SegmentCache::new())),
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

        let manifest = resolve_manifest(&self.client, &track.track_urn, token)?;

        let init_bytes = if let Some(init_url) = &manifest.init_url {
            Arc::new(self.download_bytes(init_url)?)
        } else {
            Arc::new(Vec::new())
        };

        let first_segment_bytes = if !manifest.segments.is_empty() {
            match self.download_bytes(&manifest.segments[0].url) {
                Ok(bytes) => {
                    let mut cache = SegmentCache::new();
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
                Arc::new(Mutex::new(SegmentCache::new()))
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
        position: &Arc<Mutex<Position>>,
        wave_buffer: &Arc<Mutex<std::collections::VecDeque<f32>>>,
    ) {
        let old_track_urn = position
            .lock()
            .unwrap()
            .track
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
                    position.lock().unwrap().last_start = None;
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
                        // Most likely the cached manifest's signed URLs have expired;
                        // drop it so the next attempt resolves a fresh one.
                        self.cache = None;
                        if !is_seek {
                            is_playing_flag.store(false, Ordering::SeqCst);
                            position.lock().unwrap().last_start = None;
                        }
                        return;
                    }
                }
            }
        };

        // One continuous decoder for the whole track: init + this segment now, the rest
        // streamed in by the pump as they download. Decoding each segment separately
        // restarts the AAC decoder and clicks at every boundary.
        let (bytes_tx, bytes_rx) = std::sync::mpsc::channel::<Arc<Vec<u8>>>();
        if !init_bytes.is_empty() {
            let _ = bytes_tx.send(Arc::clone(&init_bytes));
        }
        let _ = bytes_tx.send(Arc::clone(&media_bytes));

        let decoder = match Decoder::builder()
            .with_data(SegmentReader::new(bytes_rx))
            .with_seekable(false)
            .with_gapless(true)
            .build()
        {
            Ok(decoder) => decoder,
            Err(_) => {
                if !is_seek {
                    is_playing_flag.store(false, Ordering::SeqCst);
                    position.lock().unwrap().last_start = None;
                }
                return;
            }
        };
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();

        let new_sink = {
            let stream_guard = self.stream.lock().unwrap();
            Sink::connect_new(stream_guard.mixer())
        };
        new_sink.set_volume(target_volume);

        let gen_for_pump = if is_seek {
            let generation_id = self.bump_generation();
            debug_assert_eq!(generation_id, planned_generation);
            generation_id
        } else {
            planned_generation
        };

        let (pcm_tx, pcm_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(PCM_CHUNKS);
        spawn_decode_thread(
            decoder,
            Duration::from_millis(offset_within_segment_ms),
            pcm_tx,
            Arc::clone(&self.generation),
            gen_for_pump,
        );
        // EQ inside the tap, so the visualiser draws what is actually heard.
        new_sink.append(TapSource::new(
            EqSource::new(PcmSource::new(pcm_rx, channels, sample_rate)),
            Arc::clone(wave_buffer),
        ));

        let old_sink_for_fade = if is_seek && has_old_sink {
            sink_arc.lock().unwrap().take()
        } else {
            None
        };

        *sink_arc.lock().unwrap() = Some(new_sink);

        if let Some(old_sink) = old_sink_for_fade {
            crossfade_and_stop(old_sink, target_volume);
        }

        {
            let mut pos = position.lock().unwrap();
            pos.track = Some(track.clone());
            pos.elapsed = Duration::from_millis(position_ms);
            pos.last_start = Some(Instant::now());
        }
        is_playing_flag.store(true, Ordering::SeqCst);

        use crate::player::stream::downloader::SegmentPumpParams;
        spawn_segment_pump(SegmentPumpParams {
            client: self.client.clone(),
            generation: Arc::clone(&self.generation),
            generation_value: gen_for_pump,
            manifest,
            segment_cache,
            start_segment_index: segment_index,
            bytes_tx,
            is_playing_flag: Arc::clone(is_playing_flag),
            position: Arc::clone(position),
            track_urn: track.track_urn.clone(),
            token: Arc::clone(token),
        });
    }
}

/// Decodes on its own thread so a network stall never blocks the audio callback.
/// `skip` drops the start of the first segment when playback begins mid-segment.
/// Stops when the generation moves on or the output side is dropped.
fn spawn_decode_thread(
    decoder: Decoder<SegmentReader>,
    skip: Duration,
    tx: SyncSender<Vec<f32>>,
    generation: Arc<AtomicU64>,
    generation_value: u64,
) {
    std::thread::spawn(move || {
        let mut source: Box<dyn Source<Item = f32> + Send> = if skip.is_zero() {
            Box::new(decoder)
        } else {
            Box::new(decoder.skip_duration(skip))
        };
        let mut chunk = Vec::with_capacity(PCM_CHUNK_SAMPLES);
        loop {
            match source.next() {
                Some(sample) => {
                    chunk.push(sample);
                    if chunk.len() == PCM_CHUNK_SAMPLES {
                        if generation.load(Ordering::SeqCst) != generation_value {
                            return;
                        }
                        let full = std::mem::replace(&mut chunk, Vec::with_capacity(PCM_CHUNK_SAMPLES));
                        if tx.send(full).is_err() {
                            return;
                        }
                    }
                }
                None => {
                    if !chunk.is_empty() {
                        let _ = tx.send(chunk);
                    }
                    return;
                }
            }
        }
    });
}

fn crossfade_and_stop(old_sink: Sink, target_volume: f32) {
    let total_ms = CROSSFADE_DURATION.as_millis() as u64;
    let step_ms = (total_ms / CROSSFADE_STEPS as u64).max(1);

    for i in 0..=CROSSFADE_STEPS {
        let t = i as f32 / CROSSFADE_STEPS as f32;
        old_sink.set_volume(target_volume * (1.0 - t));
        std::thread::sleep(Duration::from_millis(step_ms));
    }

    old_sink.stop();
}
