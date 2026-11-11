use anyhow::Context;
use rodio::mixer::Mixer;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::collections::VecDeque;
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use reqwest::Url;

use crate::api::Track;
use crate::api::rate_limit::{RateLimited, SendGated};
use crate::auth::Token;
use crate::player::Position;
use crate::player::commands::{PlayerCommand, TrackChange};
use crate::player::stream::cache::{CachedHls, SegmentCache};
use crate::player::stream::hls::resolve_manifest;
use crate::player::stream::downloader::spawn_segment_pump;
use crate::player::stream::eq::EqSource;
use crate::player::stream::reader::{Feed, PcmSource, SegmentReader};
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

/// Opens the default output device. Fails when there is none, which the
/// caller has to hear about before the TUI takes over the terminal.
pub(crate) fn open_output_stream() -> anyhow::Result<OutputStream> {
    OutputStreamBuilder::open_default_stream().context("could not open an audio output device")
}

/// The volume the user asked for. Kept apart from the sinks, whose own volume
/// is somewhere between silence and this while a fade is running.
#[derive(Clone)]
pub(crate) struct Volume(Arc<AtomicU32>);

impl Volume {
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicU32::new(1.0f32.to_bits())))
    }

    pub(crate) fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::SeqCst))
    }

    /// Moves the volume by `delta`, within 0..=2. Only the player thread writes.
    fn step(&self, delta: f32) -> f32 {
        let volume = (self.get() + delta).clamp(0.0, 2.0);
        self.0.store(volume.to_bits(), Ordering::SeqCst);
        volume
    }
}

/// A track that could not be started, for the TUI to report and skip past.
pub struct LoadFailure {
    pub track: Track,
    /// Too many tracks in a row have failed (offline, most likely): stop
    /// skipping ahead rather than run through the whole queue.
    pub give_up: bool,
    /// SoundCloud's rate limit is why: nothing is wrong with the track, which
    /// can be started once the limit lifts.
    pub rate_limited: bool,
}

/// What the player thread shares with [`crate::player::Player`] and with the
/// threads it starts.
#[derive(Clone)]
pub(crate) struct Shared {
    pub is_playing: Arc<AtomicBool>,
    pub is_seeking: Arc<AtomicBool>,
    pub sink: Arc<Mutex<Option<Sink>>>,
    pub position: Arc<Mutex<Position>>,
    pub wave_buffer: Arc<Mutex<VecDeque<f32>>>,
    pub volume: Volume,
    pub load_failure: Arc<Mutex<Option<LoadFailure>>>,
}

impl Shared {
    pub(crate) fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            is_seeking: Arc::new(AtomicBool::new(false)),
            sink: Arc::new(Mutex::new(None)),
            position: Arc::new(Mutex::new(Position::default())),
            wave_buffer: Arc::new(Mutex::new(VecDeque::new())),
            volume: Volume::new(),
            load_failure: Arc::new(Mutex::new(None)),
        }
    }
}

/// Everything fetched for a track before it can start: the network half of a
/// play or seek, done off the player thread.
pub(crate) struct Loaded {
    hls: CachedHls,
    segment_index: usize,
    offset_within_segment_ms: u64,
    media_bytes: Arc<Vec<u8>>,
}

/// A play or seek whose audio is still being fetched.
struct Pending {
    /// Only the latest request's result is acted on.
    seq: u64,
    track: Track,
    position_ms: u64,
    /// Same track as the one playing: a reposition, not a track change.
    is_seek: bool,
    /// Fast-forward or rewind, as opposed to a play of the same track again.
    user_seek: bool,
    /// The outgoing track keeps playing, to be crossfaded out.
    overlap: bool,
    outgoing_generation: u64,
    /// What is already in hand for this track, so the fetch can skip it.
    cached: Option<CachedHls>,
}

/// What a fetch thread is sent off to get.
struct LoadJob {
    seq: u64,
    track: Track,
    position_ms: u64,
    cached: Option<CachedHls>,
}

pub(crate) struct PlaybackEngine {
    mixer: Mixer,
    client: reqwest::blocking::Client,
    token: Arc<Mutex<Token>>,
    shared: Shared,
    /// Where the fetch threads hand their results back to the player thread.
    results: Sender<PlayerCommand>,
    generation: Arc<AtomicU64>,
    /// The one superseded generation still allowed to run, because it is
    /// crossfading out behind the current track. See [`is_live`].
    fading: Arc<AtomicU64>,
    /// Generation of the track whose sink is in `shared.sink`.
    sink_generation: u64,
    /// User-chosen crossfade length in milliseconds, 0 when switched off.
    crossfade_ms: u64,
    /// Whether that crossfade also applies when the user skips tracks.
    crossfade_on_user_skips: bool,
    cache: Option<CachedHls>,
    preload_next: Option<CachedHls>,
    /// The track a preload was last asked for; a result for any other is stale.
    preload_wanted: Option<String>,
    /// The user has paused. Seeks and track changes leave it that way.
    paused: bool,
    load_seq: u64,
    pending: Option<Pending>,
    /// A fetch thread is out. Requests made meanwhile wait for it to come
    /// back, and only the latest of them is then fetched: holding down "next"
    /// costs one fetch at a time, not one per key press.
    // ponytail: a superseded fetch runs to completion (or its timeouts)
    // before the wanted one starts; give `load` a cancel check between
    // requests if skipping on a bad connection feels slow.
    loading: bool,
    consecutive_failures: u32,
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

fn download_bytes(client: &reqwest::blocking::Client, url: &Url) -> anyhow::Result<Vec<u8>> {
    let bytes = client
        .get(url.as_str())
        .send_gated()
        .with_context(|| format!("failed to download {}", url))?
        .bytes()
        .with_context(|| format!("failed to read bytes {}", url))?;
    Ok(bytes.to_vec())
}

/// Resolves a track's manifest and downloads its init segment.
fn fetch_hls(
    client: &reqwest::blocking::Client,
    track: &Track,
    token: &Arc<Mutex<Token>>,
) -> anyhow::Result<CachedHls> {
    let manifest = resolve_manifest(client, &track.track_urn, token)?;

    let init_bytes = if let Some(init_url) = &manifest.init_url {
        Arc::new(download_bytes(client, init_url)?)
    } else {
        Arc::new(Vec::new())
    };

    Ok(CachedHls {
        track_urn: track.track_urn.clone(),
        fetched_at: Instant::now(),
        manifest: Arc::new(manifest),
        init_bytes,
        segment_cache: Arc::new(Mutex::new(SegmentCache::new())),
    })
}

/// The network half of starting `track` at `position_ms`: its manifest and
/// init segment unless `cached` already has them, and the segment playback
/// starts in. Runs on its own thread; see [`PlaybackEngine::play`].
fn load(
    client: &reqwest::blocking::Client,
    track: &Track,
    position_ms: u64,
    cached: Option<CachedHls>,
    token: &Arc<Mutex<Token>>,
) -> anyhow::Result<Loaded> {
    let hls = match cached {
        Some(hls) => hls,
        None => fetch_hls(client, track, token)?,
    };
    let (segment_index, offset_within_segment_ms) = hls.manifest.locate_position(position_ms);

    let cached_bytes = hls.segment_cache.lock().unwrap().get(segment_index);
    let media_bytes = match cached_bytes {
        Some(bytes) => bytes,
        None => {
            let bytes = Arc::new(download_bytes(client, &hls.manifest.segments[segment_index].url)?);
            hls.segment_cache.lock().unwrap().insert(segment_index, Arc::clone(&bytes));
            bytes
        }
    };

    Ok(Loaded { hls, segment_index, offset_within_segment_ms, media_bytes })
}

/// Tracks failing in a row before the queue stops skipping ahead.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

impl PlaybackEngine {
    pub(crate) fn new(
        mixer: Mixer,
        token: Arc<Mutex<Token>>,
        shared: Shared,
        results: Sender<PlayerCommand>,
    ) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("sctui")
            .timeout(Duration::from_secs(15))
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self {
            mixer,
            client,
            token,
            shared,
            results,
            generation: Arc::new(AtomicU64::new(0)),
            fading: Arc::new(AtomicU64::new(NO_GENERATION)),
            sink_generation: NO_GENERATION,
            crossfade_ms: 0,
            crossfade_on_user_skips: true,
            cache: None,
            preload_next: None,
            preload_wanted: None,
            paused: false,
            load_seq: 0,
            pending: None,
            loading: false,
            consecutive_failures: 0,
        })
    }

    pub(crate) fn set_crossfade(&mut self, ms: u64, on_user_skips: bool) {
        self.crossfade_ms = ms;
        self.crossfade_on_user_skips = on_user_skips;
    }

    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub(crate) fn pause(&mut self) {
        let sink = self.shared.sink.lock().unwrap();
        if sink.is_none() && self.pending.is_none() {
            return;
        }
        self.paused = true;
        if let Some(s) = sink.as_ref() {
            s.pause();
        }
        self.shared.is_playing.store(false, Ordering::SeqCst);
        self.shared.position.lock().unwrap().stop(Instant::now());
    }

    pub(crate) fn resume(&mut self) {
        self.paused = false;
        // A track is on its way in: it starts playing when it lands. The sink still
        // here belongs to the one being replaced and must stay quiet.
        if self.pending.is_some() {
            return;
        }
        if let Some(ref s) = *self.shared.sink.lock().unwrap() {
            s.play();
            self.shared.is_playing.store(true, Ordering::SeqCst);
            self.shared.position.lock().unwrap().start(Instant::now());
        }
    }

    pub(crate) fn change_volume(&mut self, delta: f32) {
        let volume = self.shared.volume.step(delta);
        // A crossfade that is under way is already steering the sink towards
        // the user's volume, and reads it afresh at every step.
        let fading_in = self.fading.load(Ordering::SeqCst) != NO_GENERATION
            && self.pending.as_ref().is_none_or(|p| !p.overlap);
        if !fading_in && let Some(ref s) = *self.shared.sink.lock().unwrap() {
            s.set_volume(volume);
        }
    }

    /// Fast-forward or rewind by ten seconds.
    pub(crate) fn seek_by(&mut self, forward: bool) {
        // Until the track on its way has started there is nothing to seek in.
        if self.pending.as_ref().is_some_and(|p| !p.user_seek) {
            return;
        }
        let (track, current) = {
            let pos = self.shared.position.lock().unwrap();
            let Some(track) = pos.track.clone() else { return };
            (track, pos.now())
        };
        // A seek still loading has not moved the clock yet; carry on from where
        // it is headed so repeated presses add up.
        let current = match &self.pending {
            Some(p) => Duration::from_millis(p.position_ms),
            None => current,
        };
        let step = Duration::from_secs(10);

        if !forward {
            self.play(track, current.saturating_sub(step).as_millis() as u64, TrackChange::Seek);
            return;
        }
        let new_elapsed = current + step;
        let max_duration = Duration::from_millis(track.duration_ms);
        if new_elapsed >= max_duration {
            // Leave the track in the same state as a natural end-of-track
            // (is_playing still true, elapsed == duration) so the TUI tick's
            // end-of-track handling advances to the next track.
            self.pending = None;
            self.shared.is_seeking.store(false, Ordering::SeqCst);
            if let Some(ref s) = *self.shared.sink.lock().unwrap() {
                s.stop();
            }
            let mut pos = self.shared.position.lock().unwrap();
            pos.elapsed = max_duration;
            pos.last_start = None;
            pos.feed = Feed::default();
        } else {
            self.play(track, new_elapsed.as_millis() as u64, TrackChange::Seek);
        }
    }

    /// Starts fetching the track the queue will move on to, on its own thread.
    pub(crate) fn preload(&mut self, track: Track) {
        if self.preload_next.as_ref().is_some_and(|p| p.track_urn == track.track_urn) {
            return;
        }
        self.preload_wanted = Some(track.track_urn.clone());

        let client = self.client.clone();
        let token = Arc::clone(&self.token);
        let results = self.results.clone();
        std::thread::spawn(move || {
            let Ok(hls) = fetch_hls(&client, &track, &token) else { return };
            if let Ok(bytes) = download_bytes(&client, &hls.manifest.segments[0].url) {
                hls.segment_cache.lock().unwrap().insert(0, Arc::new(bytes));
            }
            let _ = results.send(PlayerCommand::Preloaded(hls));
        });
    }

    /// A preload has come back. Kept only if that track is still the one wanted.
    pub(crate) fn finish_preload(&mut self, hls: CachedHls) {
        if self.preload_wanted.as_deref() == Some(hls.track_urn.as_str()) {
            self.preload_next = Some(hls);
        }
    }

    /// Plays `track` from `position_ms`. Everything that needs the network
    /// happens on a thread of its own, which reports back with
    /// `PlayerCommand::Loaded` for [`Self::finish_load`]; the player thread
    /// goes straight back to answering pause and volume.
    pub(crate) fn play(&mut self, track: Track, position_ms: u64, change: TrackChange) {
        self.begin(track, position_ms, change);
        self.dispatch();
    }

    /// Sends a fetch thread off for the pending request, if none is out.
    fn dispatch(&mut self) {
        let Some(LoadJob { seq, track, position_ms, cached }) = self.take_job() else { return };
        let client = self.client.clone();
        let token = Arc::clone(&self.token);
        let results = self.results.clone();
        std::thread::spawn(move || {
            let result = load(&client, &track, position_ms, cached, &token);
            let _ = results.send(PlayerCommand::Loaded { seq, result });
        });
    }

    /// The fetch to start now: the pending request's, unless one is still out.
    fn take_job(&mut self) -> Option<LoadJob> {
        if self.loading {
            return None;
        }
        let pending = self.pending.as_mut()?;
        self.loading = true;
        Some(LoadJob {
            seq: pending.seq,
            track: pending.track.clone(),
            position_ms: pending.position_ms,
            cached: pending.cached.take(),
        })
    }

    /// The immediate half of a play or seek: supersedes whatever was loading
    /// and deals with the outgoing track. Returns the request's number.
    fn begin(&mut self, track: Track, position_ms: u64, change: TrackChange) -> u64 {
        let old_track_urn = self
            .shared
            .position
            .lock()
            .unwrap()
            .track
            .as_ref()
            .map(|t| t.track_urn.clone());
        // Replaying the same track (a seek, or repeat-one) is a reposition of what
        // is already playing whatever the caller thought it was asking for.
        let is_seek = old_track_urn.as_deref() == Some(&track.track_urn);
        let user_seek = change == TrackChange::Seek;
        let change = if is_seek { TrackChange::Seek } else { change };

        // A crossfade leaves the outgoing track playing while its replacement
        // loads and fades up, so it keeps its sink and its threads for now. With
        // crossfading off, a track change is still a clean stop. So is one made
        // while paused: there is nothing audible to fade.
        let has_old_sink = self.shared.sink.lock().unwrap().is_some();
        let overlap = has_old_sink
            && !self.paused
            && crossfade_applies(self.crossfade_ms, self.crossfade_on_user_skips, change);
        let outgoing_generation = self.sink_generation;

        if !is_seek {
            self.fading
                .store(if overlap { outgoing_generation } else { NO_GENERATION }, Ordering::SeqCst);
            self.bump_generation();
            if !overlap && let Some(ref s) = *self.shared.sink.lock().unwrap() {
                s.stop();
            }
            // Whatever was lined up to follow the old track is not what follows
            // this one, unless it is this one.
            self.preload_wanted = None;
            if self.preload_next.as_ref().is_some_and(|p| p.track_urn == track.track_urn) {
                self.cache = self.preload_next.take();
            }
            self.preload_next = None;
        }

        let cached = self.cache.clone().filter(|c| c.is_valid_for(&track, Instant::now()));

        self.load_seq += 1;
        self.shared.is_seeking.store(user_seek, Ordering::SeqCst);
        self.pending = Some(Pending {
            seq: self.load_seq,
            track,
            position_ms,
            is_seek,
            user_seek,
            overlap,
            outgoing_generation,
            cached,
        });
        self.load_seq
    }

    /// The network half of a play or seek has come back.
    pub(crate) fn finish_load(&mut self, seq: u64, result: anyhow::Result<Loaded>) {
        self.settle(seq, result);
        // Whatever was asked for while that fetch was out can go now.
        self.dispatch();
    }

    /// Acts on a fetch result. One for anything but the latest request is
    /// dropped: the user has moved on.
    fn settle(&mut self, seq: u64, result: anyhow::Result<Loaded>) {
        self.loading = false;
        let Some(pending) = self.pending.take_if(|p| p.seq == seq) else { return };
        self.shared.is_seeking.store(false, Ordering::SeqCst);

        let started = match result {
            Ok(loaded) => {
                self.cache = Some(loaded.hls.clone());
                self.start(&pending, loaded)
            }
            Err(e) => {
                // Most likely the cached manifest's signed URLs have expired;
                // drop it so the next attempt resolves a fresh one.
                self.cache = None;
                Err(e)
            }
        };
        if let Err(e) = started {
            self.abort_switch(pending, e.downcast_ref::<RateLimited>().is_some());
        }
    }

    /// Puts the fetched track on the output: decoder, sink, fade, clock, pump.
    fn start(&mut self, pending: &Pending, loaded: Loaded) -> anyhow::Result<()> {
        let Loaded { hls, segment_index, offset_within_segment_ms, media_bytes } = loaded;
        let track = &pending.track;
        let is_seek = pending.is_seek;
        // Paused since the switch was asked for: nothing audible to overlap.
        let overlap = pending.overlap && !self.paused;
        let sink_arc = &self.shared.sink;

        // One continuous decoder for the whole track: init + this segment now, the rest
        // streamed in by the pump as they download. Decoding each segment separately
        // restarts the AAC decoder and clicks at every boundary.
        let (bytes_tx, bytes_rx) = std::sync::mpsc::channel::<Arc<Vec<u8>>>();
        if !hls.init_bytes.is_empty() {
            let _ = bytes_tx.send(Arc::clone(&hls.init_bytes));
        }
        let _ = bytes_tx.send(Arc::clone(&media_bytes));

        // Nothing else feeds the channel until the pump starts below, so the
        // probe must not wait for more: a truncated or bogus first segment has
        // to fail here rather than hang the player thread.
        let reader = SegmentReader::new(bytes_rx);
        let wait_for_more = reader.wait_for_more();
        let decoder = Decoder::builder()
            .with_data(reader)
            .with_seekable(false)
            .with_gapless(true)
            .build()
            .context("failed to decode the first segment")?;
        wait_for_more.store(true, Ordering::SeqCst);
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();

        let new_sink = Sink::connect_new(&self.mixer);
        // A crossfade brings the new track up from silence; otherwise it starts
        // straight away at the user's volume. Never at whatever the last sink
        // happened to be at: that may have been part way through a fade.
        new_sink.set_volume(if overlap { 0.0 } else { self.shared.volume.get() });
        if self.paused {
            new_sink.pause();
        }

        // A track change moved the generation on when it was asked for; a seek
        // does so only now that its replacement is ready.
        let gen_for_pump = if is_seek {
            self.bump_generation()
        } else {
            self.generation.load(Ordering::SeqCst)
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
        let pcm = PcmSource::new(pcm_rx, channels, sample_rate);
        let feed = pcm.feed();
        // EQ inside the tap, so the visualiser draws what is actually heard.
        new_sink.append(TapSource::new(
            EqSource::new(pcm),
            Arc::clone(&self.shared.wave_buffer),
            Arc::clone(&self.generation),
            gen_for_pump,
        ));

        // Swapped under one lock: a fade still running for the previous change
        // checks the generation under the same lock before touching the sink.
        let old_sink = sink_arc.lock().unwrap().replace(new_sink);
        self.sink_generation = gen_for_pump;

        if let Some(old_sink) = old_sink.filter(|_| is_seek || pending.overlap) {
            spawn_fade(Fade {
                old_sink,
                // A seek only needs the click taken off the end of the old sink;
                // a track change fades the new one up as the old one goes down.
                incoming: overlap.then(|| Arc::clone(sink_arc)),
                duration: if overlap { Duration::from_millis(self.crossfade_ms) } else { CROSSFADE_DURATION },
                volume: self.shared.volume.clone(),
                is_playing_flag: Arc::clone(&self.shared.is_playing),
                generation: Arc::clone(&self.generation),
                incoming_generation: gen_for_pump,
                fading: Arc::clone(&self.fading),
                fading_value: pending.outgoing_generation,
            });
        }

        {
            let mut pos = self.shared.position.lock().unwrap();
            pos.track = Some(track.clone());
            pos.elapsed = Duration::from_millis(pending.position_ms);
            // Paused stays paused: the clock waits for resume, like the sink.
            pos.last_start = (!self.paused).then(Instant::now);
            pos.feed = feed;
        }
        self.shared.is_playing.store(!self.paused, Ordering::SeqCst);
        self.consecutive_failures = 0;

        use crate::player::stream::downloader::SegmentPumpParams;
        spawn_segment_pump(SegmentPumpParams {
            client: self.client.clone(),
            generation: Arc::clone(&self.generation),
            fading: Arc::clone(&self.fading),
            generation_value: gen_for_pump,
            manifest: hls.manifest,
            segment_cache: hls.segment_cache,
            start_segment_index: segment_index,
            bytes_tx,
            position: Arc::clone(&self.shared.position),
            track_urn: track.track_urn.clone(),
            token: Arc::clone(&self.token),
        });
        Ok(())
    }

    /// A play that failed part way through. The outgoing track may still be
    /// playing — a crossfade only stops it once its replacement is running — so
    /// stop it, report the player as idle, and tell the TUI which track failed
    /// so it can say so and move on. A failed fast-forward or rewind changed
    /// nothing, so it leaves the track that is playing alone. A track held
    /// back by the rate limit is no sign of being offline, so it does not
    /// count towards giving up.
    fn abort_switch(&mut self, pending: Pending, rate_limited: bool) {
        if pending.user_seek {
            return;
        }
        if let Some(sink) = self.shared.sink.lock().unwrap().as_ref() {
            sink.stop();
        }
        self.fading.store(NO_GENERATION, Ordering::SeqCst);
        self.shared.is_playing.store(false, Ordering::SeqCst);
        self.shared.position.lock().unwrap().stop(Instant::now());

        self.consecutive_failures += !rate_limited as u32;
        *self.shared.load_failure.lock().unwrap() = Some(LoadFailure {
            track: pending.track,
            give_up: self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES,
            rate_limited,
        });
    }
}

/// How many samples to drop to start `skip` into the decoded stream: always a
/// whole number of frames. rodio's `skip_duration` rounds to a sample, not a
/// frame, and an odd count leaves left and right swapped from then on.
fn samples_to_skip(skip: Duration, sample_rate: u32, channels: u16) -> usize {
    let frames = skip.as_nanos() * sample_rate as u128 / 1_000_000_000;
    frames as usize * channels as usize
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
        let mut source = decoder;
        for _ in 0..samples_to_skip(skip, source.sample_rate(), source.channels()) {
            if source.next().is_none() {
                break;
            }
        }
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
    /// The volume the incoming track should end up at: the user's, read
    /// afresh at each step so a change made mid-fade is followed.
    volume: Volume,
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

/// Sink volumes for the outgoing and incoming tracks `t` of the way through a
/// fade. The outgoing one comes down from wherever it stood when the fade
/// began — part way up, if it was itself still fading in — and the incoming
/// one heads for the user's volume, never for what another sink was left at.
fn fade_levels(outgoing_start: f32, user_volume: f32, t: f32, overlapping: bool) -> (f32, f32) {
    let (out_gain, in_gain) = fade_gains(t, overlapping);
    (outgoing_start * out_gain, user_volume * in_gain)
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
        let outgoing_start = fade.old_sink.volume();
        let mut interrupted = false;
        for i in 0..=steps {
            // Pausing has to take the outgoing track with it; it is no longer
            // the sink the player thread reaches for.
            if fade.is_playing_flag.load(Ordering::SeqCst) {
                fade.old_sink.play();
            } else {
                fade.old_sink.pause();
            }

            let (out_level, in_level) = fade_levels(
                outgoing_start,
                fade.volume.get(),
                i as f32 / steps as f32,
                fade.incoming.is_some(),
            );
            fade.old_sink.set_volume(out_level);
            if let Some(sink_arc) = &fade.incoming {
                // Checked under the lock the sink is swapped under, so this
                // never touches a sink that belongs to a later change.
                let sink = sink_arc.lock().unwrap();
                if fade.generation.load(Ordering::SeqCst) != fade.incoming_generation {
                    // Another track has taken over mid-fade and owns the volume
                    // of whatever is in the sink now.
                    interrupted = true;
                    break;
                }
                if let Some(sink) = sink.as_ref() {
                    sink.set_volume(in_level);
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
        // A volume change during the last step saw the fade still running and
        // left the sink to it.
        if !interrupted && let Some(sink_arc) = &fade.incoming {
            let sink = sink_arc.lock().unwrap();
            if fade.generation.load(Ordering::SeqCst) == fade.incoming_generation
                && let Some(sink) = sink.as_ref()
            {
                sink.set_volume(fade.volume.get());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::stream::hls::{HlsManifest, HlsSegment};
    use rodio::mixer::MixerSource;
    use std::sync::mpsc::Receiver;

    fn track(urn: &str) -> Track {
        Track {
            title: urn.into(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 60_000,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: urn.into(),
        }
    }

    /// Half a second of stereo 16-bit PCM in a WAV: something the decoder can
    /// probe and play without the network or an AAC fixture.
    fn wav() -> Vec<u8> {
        let frames = 22_050u32;
        let data_len = frames * 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&2u16.to_le_bytes()); // stereo
        out.extend_from_slice(&44_100u32.to_le_bytes());
        out.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for _ in 0..frames {
            out.extend_from_slice(&8_000i16.to_le_bytes());
            out.extend_from_slice(&(-8_000i16).to_le_bytes());
        }
        out
    }

    fn hls(urn: &str) -> CachedHls {
        CachedHls {
            track_urn: urn.into(),
            fetched_at: Instant::now(),
            // One segment, so the pump has nothing to download and the stream
            // simply ends after it.
            manifest: Arc::new(HlsManifest {
                init_url: None,
                segments: vec![HlsSegment { url: Url::parse("https://cdn.test/0").unwrap() }],
                segment_start_ms: vec![0],
                total_duration_ms: 60_000,
            }),
            init_bytes: Arc::new(Vec::new()),
            segment_cache: Arc::new(Mutex::new(SegmentCache::new())),
        }
    }

    fn loaded(urn: &str, position_ms: u64, bytes: Vec<u8>) -> Loaded {
        Loaded {
            hls: hls(urn),
            segment_index: 0,
            offset_within_segment_ms: position_ms,
            media_bytes: Arc::new(bytes),
        }
    }

    /// An engine playing into a mixer nobody listens to: no audio device. The
    /// mixer's output is handed back so a test can pull samples through it,
    /// which is all an output device does.
    fn engine() -> (PlaybackEngine, MixerSource, Receiver<PlayerCommand>) {
        let (mixer, output) = rodio::mixer::mixer(2, 44_100);
        let (tx, rx) = std::sync::mpsc::channel();
        let token = Token { access_token: String::new(), refresh_token: String::new(), obtained_at: 0 };
        let engine = PlaybackEngine::new(mixer, Arc::new(Mutex::new(token)), Shared::new(), tx).unwrap();
        (engine, output, rx)
    }

    /// A play or seek whose fetch comes straight back with playable audio.
    fn play(engine: &mut PlaybackEngine, urn: &str, position_ms: u64, change: TrackChange) {
        let seq = engine.begin(track(urn), position_ms, change);
        engine.settle(seq, Ok(loaded(urn, position_ms, wav())));
    }

    fn fail(engine: &mut PlaybackEngine, urn: &str, position_ms: u64, change: TrackChange) {
        let seq = engine.begin(track(urn), position_ms, change);
        engine.settle(seq, Err(anyhow::anyhow!("403")));
    }

    fn sink_volume(engine: &PlaybackEngine) -> f32 {
        engine.shared.sink.lock().unwrap().as_ref().unwrap().volume()
    }

    fn sink_paused(engine: &PlaybackEngine) -> bool {
        engine.shared.sink.lock().unwrap().as_ref().unwrap().is_paused()
    }

    fn playing_urn(engine: &PlaybackEngine) -> Option<String> {
        engine.shared.position.lock().unwrap().track.as_ref().map(|t| t.track_urn.clone())
    }

    #[test]
    fn a_seek_during_a_crossfade_starts_at_the_users_volume() {
        let (mut engine, _output, _rx) = engine();
        engine.set_crossfade(10_000, true);
        engine.change_volume(-0.2);
        play(&mut engine, "a", 0, TrackChange::UserSkip);
        play(&mut engine, "b", 0, TrackChange::UserSkip);
        assert!(sink_volume(&engine) < 0.2, "b is only just starting to fade in");

        play(&mut engine, "b", 100, TrackChange::Seek);
        assert_eq!(sink_volume(&engine), 0.8, "not the level the fade had reached");
        assert_eq!(engine.shared.volume.get(), 0.8);
    }

    #[test]
    fn a_fade_heads_for_the_users_volume_from_wherever_the_outgoing_track_stood() {
        // The outgoing track was itself only 30% of the way up.
        assert_eq!(fade_levels(0.3, 0.8, 0.0, true), (0.3, 0.0));
        assert_eq!(fade_levels(0.3, 0.8, 1.0, true), (0.0, 0.8));
        // A seek's click-avoidance fade leaves the incoming sink alone.
        assert_eq!(fade_levels(0.8, 0.8, 0.5, false).0, 0.4);
    }

    #[test]
    fn volume_steps_stay_within_bounds_without_a_sink() {
        let volume = Volume::new();
        assert_eq!(volume.get(), 1.0);
        for _ in 0..30 {
            volume.step(0.1);
        }
        assert_eq!(volume.get(), 2.0);
        for _ in 0..30 {
            volume.step(-0.1);
        }
        assert_eq!(volume.get(), 0.0);
    }

    #[test]
    fn a_seek_while_paused_stays_paused_at_the_new_position() {
        let (mut engine, _output, _rx) = engine();
        play(&mut engine, "a", 0, TrackChange::UserSkip);
        assert!(engine.shared.is_playing.load(Ordering::SeqCst));
        engine.pause();

        play(&mut engine, "a", 200, TrackChange::Seek);
        assert!(!engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(sink_paused(&engine));
        let pos = engine.shared.position.lock().unwrap();
        assert!(pos.last_start.is_none(), "the clock is not running");
        assert_eq!(pos.elapsed, Duration::from_millis(200));
    }

    #[test]
    fn a_skip_while_paused_starts_the_new_track_paused_without_a_crossfade() {
        let (mut engine, _output, _rx) = engine();
        engine.set_crossfade(5_000, true);
        play(&mut engine, "a", 0, TrackChange::UserSkip);
        engine.pause();

        let seq = engine.begin(track("b"), 0, TrackChange::UserSkip);
        assert!(!engine.pending.as_ref().unwrap().overlap, "nothing audible to fade out");
        assert_eq!(engine.fading.load(Ordering::SeqCst), NO_GENERATION);
        engine.settle(seq, Ok(loaded("b", 0, wav())));

        assert_eq!(playing_urn(&engine).as_deref(), Some("b"));
        assert!(!engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(sink_paused(&engine));
        assert_eq!(sink_volume(&engine), 1.0, "ready at full volume, not waiting on a fade");

        engine.resume();
        assert!(engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(!sink_paused(&engine));
        assert!(engine.shared.position.lock().unwrap().last_start.is_some());
    }

    /// Enter on a row while paused: the play is followed by a resume, so the chosen
    /// track starts playing, and the paused one is not heard in the meantime.
    #[test]
    fn choosing_a_track_while_paused_plays_it_once_resumed() {
        let (mut engine, _output, _rx) = engine();
        play(&mut engine, "a", 0, TrackChange::UserSkip);
        engine.pause();

        let seq = engine.begin(track("b"), 0, TrackChange::UserSkip);
        engine.resume();
        let old_audible = engine.shared.sink.lock().unwrap().as_ref().is_some_and(|s| !s.is_paused() && !s.empty());
        assert!(!old_audible, "the paused track must not start up again while the new one loads");
        engine.settle(seq, Ok(loaded("b", 0, wav())));

        assert_eq!(playing_urn(&engine).as_deref(), Some("b"));
        assert!(engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(!sink_paused(&engine));
    }

    #[test]
    fn pausing_while_a_track_is_still_loading_starts_it_paused() {
        let (mut engine, _output, _rx) = engine();
        let seq = engine.begin(track("a"), 0, TrackChange::UserSkip);
        // The fetch is in flight; the player thread is free to take this.
        engine.pause();
        engine.settle(seq, Ok(loaded("a", 0, wav())));
        assert!(!engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(sink_paused(&engine));
    }

    #[test]
    fn a_track_that_fails_to_load_is_reported_and_leaves_the_player_idle() {
        let (mut engine, _output, _rx) = engine();
        play(&mut engine, "a", 0, TrackChange::UserSkip);
        fail(&mut engine, "b", 0, TrackChange::Natural);

        assert!(!engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(engine.shared.position.lock().unwrap().last_start.is_none());
        let failure = engine.shared.load_failure.lock().unwrap().take().expect("reported");
        assert_eq!(failure.track.track_urn, "b");
        assert!(!failure.give_up);
        assert!(engine.shared.load_failure.lock().unwrap().is_none(), "reported once");
    }

    #[test]
    fn a_run_of_failures_gives_up_and_a_success_starts_the_count_again() {
        let (mut engine, _output, _rx) = engine();
        let mut give_up = Vec::new();
        for urn in ["a", "b", "c"] {
            fail(&mut engine, urn, 0, TrackChange::UserSkip);
            give_up.push(engine.shared.load_failure.lock().unwrap().take().unwrap().give_up);
        }
        assert_eq!(give_up, [false, false, true]);

        play(&mut engine, "d", 0, TrackChange::UserSkip);
        fail(&mut engine, "e", 0, TrackChange::UserSkip);
        assert!(!engine.shared.load_failure.lock().unwrap().take().unwrap().give_up);
    }

    /// #55: a 429 is reported as such, so the TUI holds the track for when the
    /// limit lifts instead of skipping on, and it never counts towards giving up.
    #[test]
    fn a_rate_limited_track_is_reported_as_held_not_as_broken() {
        let (mut engine, _output, _rx) = engine();
        for urn in ["a", "b", "c", "d"] {
            let seq = engine.begin(track(urn), 0, TrackChange::Natural);
            let refused = anyhow::Error::from(RateLimited { until: chrono::Utc::now() });
            engine.settle(seq, Err(refused.context("failed to fetch streams endpoint")));
            let failure = engine.shared.load_failure.lock().unwrap().take().expect("reported");
            assert!(failure.rate_limited && !failure.give_up, "{urn}");
        }
        fail(&mut engine, "e", 0, TrackChange::Natural);
        let failure = engine.shared.load_failure.lock().unwrap().take().unwrap();
        assert!(!failure.rate_limited && !failure.give_up);
    }

    #[test]
    fn a_failed_replay_is_reported_but_a_failed_seek_changes_nothing() {
        let (mut engine, _output, _rx) = engine();
        play(&mut engine, "a", 0, TrackChange::UserSkip);

        // Fast-forward that could not load: the track carries on where it was.
        fail(&mut engine, "a", 10_000, TrackChange::Seek);
        assert!(engine.shared.is_playing.load(Ordering::SeqCst));
        assert!(engine.shared.load_failure.lock().unwrap().is_none());
        assert!(!engine.shared.is_seeking.load(Ordering::SeqCst));

        // Repeat-one starting the same track again: that is a play, and its
        // failure must not leave the clock running past the end.
        fail(&mut engine, "a", 0, TrackChange::Natural);
        assert!(!engine.shared.is_playing.load(Ordering::SeqCst));
        assert_eq!(engine.shared.load_failure.lock().unwrap().take().unwrap().track.track_urn, "a");
    }

    #[test]
    fn a_first_segment_that_is_not_audio_fails_instead_of_hanging() {
        let (mut engine, _output, _rx) = engine();
        let seq = engine.begin(track("a"), 0, TrackChange::UserSkip);
        // A 200 with an error page for a body.
        engine.settle(seq, Ok(loaded("a", 0, b"<html>Service Unavailable</html>".to_vec())));
        assert!(engine.shared.load_failure.lock().unwrap().is_some());
        assert_eq!(playing_urn(&engine), None);
    }

    #[test]
    fn only_the_latest_request_is_acted_on_when_its_fetch_comes_back() {
        let (mut engine, _output, _rx) = engine();
        let first = engine.begin(track("a"), 0, TrackChange::UserSkip);
        let second = engine.begin(track("b"), 0, TrackChange::UserSkip);

        engine.settle(first, Ok(loaded("a", 0, wav())));
        assert_eq!(playing_urn(&engine), None, "a was skipped past while it loaded");
        engine.settle(first, Err(anyhow::anyhow!("timed out")));
        assert!(engine.shared.load_failure.lock().unwrap().is_none(), "nor is its failure news");

        engine.settle(second, Ok(loaded("b", 0, wav())));
        assert_eq!(playing_urn(&engine).as_deref(), Some("b"));
    }

    #[test]
    fn plays_made_while_a_fetch_is_out_wait_and_only_the_last_is_fetched() {
        let (mut engine, _output, _rx) = engine();
        let a = engine.begin(track("a"), 0, TrackChange::UserSkip);
        assert_eq!(engine.take_job().map(|job| job.seq), Some(a));

        // Holding "next" down while a is still being fetched.
        engine.begin(track("b"), 0, TrackChange::UserSkip);
        assert!(engine.take_job().is_none(), "one fetch at a time");
        let c = engine.begin(track("c"), 0, TrackChange::UserSkip);
        assert!(engine.take_job().is_none());

        // Pause is answered meanwhile; nothing waits on the network.
        engine.pause();
        assert!(engine.paused);
        engine.resume();

        engine.settle(a, Ok(loaded("a", 0, wav())));
        let job = engine.take_job().expect("the fetch for the latest request starts now");
        assert_eq!((job.seq, job.track.track_urn.as_str()), (c, "c"));
        engine.settle(c, Ok(loaded("c", 0, wav())));
        assert_eq!(playing_urn(&engine).as_deref(), Some("c"));
        assert!(engine.take_job().is_none(), "nothing left to fetch");
    }

    #[test]
    fn a_preload_is_kept_only_while_its_track_is_still_next() {
        let (mut engine, _output, _rx) = engine();
        play(&mut engine, "a", 0, TrackChange::UserSkip);

        // Still next when the fetch comes back: used by the play that follows.
        engine.preload_wanted = Some("b".into());
        engine.finish_preload(hls("b"));
        engine.begin(track("b"), 0, TrackChange::Natural);
        let cached = engine.pending.as_ref().unwrap().cached.as_ref();
        assert_eq!(cached.map(|c| c.track_urn.as_str()), Some("b"));

        // The user went somewhere else while "c" was being fetched.
        engine.preload_wanted = Some("c".into());
        engine.begin(track("z"), 0, TrackChange::UserSkip);
        engine.finish_preload(hls("c"));
        assert!(engine.preload_next.is_none());
    }

    #[test]
    fn a_stream_that_runs_out_early_ends_the_track_before_the_clock_does() {
        let (mut engine, mut output, _rx) = engine();
        // Half a second of audio for a track the API says lasts a minute.
        play(&mut engine, "a", 0, TrackChange::UserSkip);
        let feed = engine.shared.position.lock().unwrap().feed.clone();
        assert!(!feed.ended());

        // Stand in for the output device: pull audio until the track is done.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !feed.ended() && Instant::now() < deadline {
            for _ in 0..4_410 {
                output.next();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(feed.ended(), "decoder finished and buffer drained");
        assert!(engine.shared.position.lock().unwrap().now() < Duration::from_secs(60));
    }

    #[test]
    fn a_mid_segment_start_skips_whole_frames() {
        // rodio's own skip drops nanos / (1e9 / rate / channels) samples, which
        // comes out odd for offsets like these and swaps the channels.
        for ms in [1u64, 3, 7, 1_234, 4_999, 9_999] {
            let skip = Duration::from_millis(ms);
            assert_eq!(samples_to_skip(skip, 44_100, 2) % 2, 0, "{ms} ms");
            assert_eq!(samples_to_skip(skip, 48_000, 6) % 6, 0, "{ms} ms");
        }
        assert_eq!(samples_to_skip(Duration::from_millis(500), 44_100, 2), 44_100);
        assert_eq!(samples_to_skip(Duration::ZERO, 44_100, 2), 0);
    }


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
