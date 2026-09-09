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
use crate::player::commands::TrackChange;
use crate::player::stream::cache::{CachedHls, SegmentCache};
use crate::player::stream::hls::{HlsManifest, resolve_manifest};
use crate::player::stream::downloader::spawn_segment_pump;
use crate::player::stream::eq::EqSource;
use crate::player::stream::reader::{PcmSource, SegmentReader};
use crate::player::stream::sample::TapSource;

/// Shortest fade used at a track change: long enough to lose the click of a
/// sink stopping mid-waveform, short enough to pass for an instant cut.
pub(crate) const CROSSFADE_DURATION: Duration = Duration::from_millis(35);
/// One volume step per 5 ms, up to this many, so a 12 s crossfade is a few
/// hundred steps rather than a few thousand.
const MAX_FADE_STEPS: u64 = 240;
/// No generation: generations start at 1, so this never matches a real one.
const NO_GENERATION: u64 = 0;
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
    /// The one superseded generation still allowed to run, because it is
    /// crossfading out behind the current track. See [`is_live`].
    fading: Arc<AtomicU64>,
    /// User-chosen crossfade length in milliseconds, 0 when switched off.
    crossfade_ms: u64,
    /// Whether that crossfade also applies when the user skips tracks.
    crossfade_on_user_skips: bool,
    cache: Option<CachedHls>,
    preload_next: Option<CachedHls>,
}

/// Whether the decode and download threads of `generation_value` are still
/// wanted. Everything belonging to a replaced track normally stops the moment
/// the generation moves on; during a crossfade the outgoing track has to keep
/// decoding and downloading until its fade ends, or it would run out of
/// buffered audio part way through.
pub(crate) fn is_live(generation: &AtomicU64, fading: &AtomicU64, generation_value: u64) -> bool {
    generation.load(Ordering::SeqCst) == generation_value
        || fading.load(Ordering::SeqCst) == generation_value
}

/// Whether this particular change should overlap the outgoing track with the
/// user's crossfade. A seek never does — it keeps the 35 ms click-avoidance
/// fade it has always had, whatever the settings say.
pub(crate) fn crossfade_applies(crossfade_ms: u64, on_user_skips: bool, change: TrackChange) -> bool {
    crossfade_ms > 0
        && match change {
            TrackChange::Natural => true,
            TrackChange::UserSkip => on_user_skips,
            TrackChange::Seek => false,
        }
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
            fading: Arc::new(AtomicU64::new(NO_GENERATION)),
            crossfade_ms: 0,
            crossfade_on_user_skips: true,
            cache: None,
            preload_next: None,
        })
    }

    pub(crate) fn set_crossfade(&mut self, ms: u64, on_user_skips: bool) {
        self.crossfade_ms = ms;
        self.crossfade_on_user_skips = on_user_skips;
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
        change: TrackChange,
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
        // Replaying the same track (a seek, or repeat-one) is a reposition of what
        // is already playing whatever the caller thought it was asking for.
        let is_seek = old_track_urn.as_deref() == Some(&track.track_urn);
        let change = if is_seek { TrackChange::Seek } else { change };

        let (has_old_sink, target_volume) = {
            let guard = sink_arc.lock().unwrap();
            let vol = guard.as_ref().map(|s| s.volume()).unwrap_or(1.0);
            (guard.is_some(), vol)
        };

        // A crossfade leaves the outgoing track playing while its replacement
        // loads and fades up, so it keeps its sink and its threads for now. With
        // crossfading off, a track change is still a clean stop.
        let crossfade = Duration::from_millis(self.crossfade_ms);
        let overlap =
            has_old_sink && crossfade_applies(self.crossfade_ms, self.crossfade_on_user_skips, change);
        let outgoing_generation = self.current_generation();
        let fading = Arc::clone(&self.fading);

        let planned_generation = outgoing_generation.wrapping_add(1);
        if !is_seek {
            if overlap {
                fading.store(outgoing_generation, Ordering::SeqCst);
            }
            let generation_id = self.bump_generation();
            debug_assert_eq!(generation_id, planned_generation);
            if !overlap && let Some(ref s) = *sink_arc.lock().unwrap() {
                s.stop();
            }
        }

        let (manifest, init_bytes, segment_cache) = match self.ensure_cached_hls(track, token) {
            Ok(v) => v,
            Err(_) => {
                abort_switch(is_seek, sink_arc, is_playing_flag, position, &fading);
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
                        abort_switch(is_seek, sink_arc, is_playing_flag, position, &fading);
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
                abort_switch(is_seek, sink_arc, is_playing_flag, position, &fading);
                return;
            }
        };
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();

        let new_sink = {
            let stream_guard = self.stream.lock().unwrap();
            Sink::connect_new(stream_guard.mixer())
        };
        // A crossfade brings the new track up from silence; otherwise it starts
        // straight away at whatever volume the last one was playing at.
        new_sink.set_volume(if overlap { 0.0 } else { target_volume });

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
            Arc::clone(&self.fading),
            gen_for_pump,
        );
        // EQ inside the tap, so the visualiser draws what is actually heard.
        new_sink.append(TapSource::new(
            EqSource::new(PcmSource::new(pcm_rx, channels, sample_rate)),
            Arc::clone(wave_buffer),
            Arc::clone(&self.generation),
            gen_for_pump,
        ));

        let old_sink_for_fade = if (is_seek || overlap) && has_old_sink {
            sink_arc.lock().unwrap().take()
        } else {
            None
        };

        *sink_arc.lock().unwrap() = Some(new_sink);

        if let Some(old_sink) = old_sink_for_fade {
            spawn_fade(Fade {
                old_sink,
                // A seek only needs the click taken off the end of the old sink;
                // a track change fades the new one up as the old one goes down.
                incoming: overlap.then(|| Arc::clone(sink_arc)),
                duration: if overlap { crossfade } else { CROSSFADE_DURATION },
                target_volume,
                is_playing_flag: Arc::clone(is_playing_flag),
                generation: Arc::clone(&self.generation),
                incoming_generation: gen_for_pump,
                fading,
                fading_value: outgoing_generation,
            });
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
            fading: Arc::clone(&self.fading),
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
    fading: Arc<AtomicU64>,
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
                        if !is_live(&generation, &fading, generation_value) {
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

struct Fade {
    /// The track being replaced: stopped and dropped once the fade ends, which
    /// is what finally releases its decode and download threads.
    old_sink: Sink,
    /// The sink to bring up at the same time, for a real crossfade. `None` for
    /// a seek, where the replacement is already at full volume.
    incoming: Option<Arc<Mutex<Option<Sink>>>>,
    duration: Duration,
    /// The volume the pair should end up at: whatever the user had set when the
    /// switch started.
    // ponytail: a volume change made during a long crossfade is overwritten
    // until the fade ends; re-read it from the incoming sink each step if that
    // ever grates.
    target_volume: f32,
    is_playing_flag: Arc<std::sync::atomic::AtomicBool>,
    generation: Arc<AtomicU64>,
    /// Generation of the incoming track. Once the generation moves past it the
    /// sink in `incoming` belongs to someone else and is left alone.
    incoming_generation: u64,
    /// Reprieve granted to the outgoing generation, released when the fade ends.
    fading: Arc<AtomicU64>,
    fading_value: u64,
}

/// Volume multipliers for the outgoing and incoming tracks `t` of the way
/// through a fade. Two tracks playing at once are ramped on an equal-power
/// curve, because summing uncorrelated audio through linear ramps dips
/// audibly in the middle; a lone outgoing track just ramps straight down.
fn fade_gains(t: f32, overlapping: bool) -> (f32, f32) {
    if overlapping {
        ((1.0 - t).sqrt(), t.sqrt())
    } else {
        (1.0 - t, 1.0)
    }
}

/// How many volume steps to take over `duration`, and how long to hold each.
fn fade_schedule(duration: Duration) -> (u64, Duration) {
    let total_ms = (duration.as_millis() as u64).max(1);
    let steps = (total_ms / 5).clamp(1, MAX_FADE_STEPS);
    (steps, Duration::from_millis((total_ms / steps).max(1)))
}

/// Runs the fade off the player thread: a 12 s crossfade must not stop it
/// answering pause, skip or volume for 12 seconds.
fn spawn_fade(fade: Fade) {
    std::thread::spawn(move || {
        let (steps, step) = fade_schedule(fade.duration);
        for i in 0..=steps {
            // Pausing has to take the outgoing track with it; it is no longer
            // the sink the player thread reaches for.
            if fade.is_playing_flag.load(Ordering::SeqCst) {
                fade.old_sink.play();
            } else {
                fade.old_sink.pause();
            }

            let (out_gain, in_gain) = fade_gains(i as f32 / steps as f32, fade.incoming.is_some());
            fade.old_sink.set_volume(fade.target_volume * out_gain);
            if let Some(sink_arc) = &fade.incoming {
                if fade.generation.load(Ordering::SeqCst) != fade.incoming_generation {
                    // Another track has taken over mid-fade and owns the volume
                    // of whatever is in the sink now.
                    break;
                }
                if let Some(sink) = sink_arc.lock().unwrap().as_ref() {
                    sink.set_volume(fade.target_volume * in_gain);
                }
            }
            std::thread::sleep(step);
        }

        fade.old_sink.stop();
        let _ = fade.fading.compare_exchange(
            fade.fading_value,
            NO_GENERATION,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    });
}

/// A track change that failed part way through. The outgoing track may still be
/// playing — a crossfade only stops it once its replacement is running — so stop
/// it and report the player as idle. The sink itself stays in place: it carries
/// the volume the next track will start at. A failed seek changed nothing, so it
/// leaves the track that is playing alone.
fn abort_switch(
    is_seek: bool,
    sink_arc: &Arc<Mutex<Option<Sink>>>,
    is_playing_flag: &std::sync::atomic::AtomicBool,
    position: &Mutex<Position>,
    fading: &AtomicU64,
) {
    if is_seek {
        return;
    }
    if let Some(sink) = sink_arc.lock().unwrap().as_ref() {
        sink.stop();
    }
    fading.store(NO_GENERATION, Ordering::SeqCst);
    is_playing_flag.store(false, Ordering::SeqCst);
    position.lock().unwrap().last_start = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crossfade_holds_a_steady_level_and_ends_swapped_over() {
        let (out0, in0) = fade_gains(0.0, true);
        assert!((out0 - 1.0).abs() < 1e-6 && in0 == 0.0, "starts on the outgoing track");
        let (out1, in1) = fade_gains(1.0, true);
        assert!(out1 == 0.0 && (in1 - 1.0).abs() < 1e-6, "ends on the incoming one");
        for step in 0..=10 {
            let (out, inc) = fade_gains(step as f32 / 10.0, true);
            let power = out * out + inc * inc;
            assert!((power - 1.0).abs() < 1e-5, "dip at t={step}: power {power}");
        }
    }

    #[test]
    fn a_fade_with_nothing_coming_in_just_ramps_down() {
        assert_eq!(fade_gains(0.5, false), (0.5, 1.0));
        assert_eq!(fade_gains(1.0, false), (0.0, 1.0));
    }

    #[test]
    fn fade_steps_stay_bounded_and_span_the_whole_duration() {
        // The click-avoidance fade keeps its original 5 ms steps.
        assert_eq!(fade_schedule(CROSSFADE_DURATION), (7, Duration::from_millis(5)));
        for ms in [1u64, 35, 500, 1_000, 12_000] {
            let (steps, step) = fade_schedule(Duration::from_millis(ms));
            assert!(steps <= MAX_FADE_STEPS, "{ms} ms takes {steps} steps");
            let elapsed = steps * step.as_millis() as u64;
            assert!(elapsed <= ms.max(1) && elapsed * 2 >= ms, "{ms} ms fade ran for {elapsed} ms");
        }
    }

    #[test]
    fn the_crossfade_setting_decides_which_changes_overlap() {
        for change in [TrackChange::Natural, TrackChange::UserSkip, TrackChange::Seek] {
            assert!(!crossfade_applies(0, true, change), "crossfading off: {change:?}");
        }
        // On for everything: a natural handover and a skip both overlap.
        assert!(crossfade_applies(5_000, true, TrackChange::Natural));
        assert!(crossfade_applies(5_000, true, TrackChange::UserSkip));
        // Off for skips: only a track ending of its own accord overlaps.
        assert!(crossfade_applies(5_000, false, TrackChange::Natural));
        assert!(!crossfade_applies(5_000, false, TrackChange::UserSkip));
        // A seek is untouched by the setting either way.
        assert!(!crossfade_applies(5_000, true, TrackChange::Seek));
        assert!(!crossfade_applies(5_000, false, TrackChange::Seek));
    }

    #[test]
    fn only_the_current_and_fading_out_generations_are_live() {
        let generation = AtomicU64::new(4);
        let fading = AtomicU64::new(NO_GENERATION);
        assert!(is_live(&generation, &fading, 4));
        assert!(!is_live(&generation, &fading, 3));

        // Track 3 crossfading out behind track 4 keeps decoding until its fade ends.
        fading.store(3, Ordering::SeqCst);
        assert!(is_live(&generation, &fading, 3));
        assert!(!is_live(&generation, &fading, 2));
        fading.store(NO_GENERATION, Ordering::SeqCst);
        assert!(!is_live(&generation, &fading, 3));
    }
}
