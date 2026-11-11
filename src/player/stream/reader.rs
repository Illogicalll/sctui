//! Glue for decoding an HLS track with a single continuous decoder.
//!
//! Decoding every segment on its own restarts the AAC decoder each time. AAC
//! frames overlap (MDCT), so a cold start mangles the first frame of every
//! segment: an audible tick every ~10 s. Instead, [`SegmentReader`] presents
//! `init + seg0 + seg1 + …` as one byte stream to one decoder, and
//! [`PcmSource`] carries the decoded samples to the audio thread without
//! letting network stalls block it.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;

/// Byte stream stitched from chunks arriving on a channel. Read-only forward
/// stream; the decoder is told it is not seekable.
pub(crate) struct SegmentReader {
    // Mutex only because rodio wants `Sync`; the decoder is the sole reader.
    rx: Mutex<Receiver<Arc<Vec<u8>>>>,
    current: Option<Arc<Vec<u8>>>,
    offset: usize,
    position: u64,
    /// Off while the decoder is being built: the probe runs on the thread that
    /// holds the only sender, so waiting there for more bytes would never end.
    /// Until this is switched on, running out of chunks reads as end of stream.
    wait_for_more: Arc<AtomicBool>,
}

impl SegmentReader {
    pub(crate) fn new(rx: Receiver<Arc<Vec<u8>>>) -> Self {
        Self {
            rx: Mutex::new(rx),
            current: None,
            offset: 0,
            position: 0,
            wait_for_more: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set to `true` once the decoder is built and something else is feeding
    /// the channel; from then on a read waits for the next chunk.
    pub(crate) fn wait_for_more(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.wait_for_more)
    }

    /// Moves on to the next chunk, waiting for it once `wait_for_more` is on.
    /// `false` when there is none: the sender is gone, or nothing is queued
    /// and waiting is still off.
    fn advance(&mut self) -> bool {
        let rx = self.rx.lock().unwrap();
        let next = if self.wait_for_more.load(Ordering::SeqCst) {
            rx.recv().ok()
        } else {
            rx.try_recv().ok()
        };
        drop(rx);
        match next {
            Some(chunk) => {
                self.current = Some(chunk);
                self.offset = 0;
                true
            }
            None => false,
        }
    }
}

impl Read for SegmentReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Some(chunk) = &self.current {
                let remaining = &chunk[self.offset..];
                if !remaining.is_empty() {
                    let n = remaining.len().min(buf.len());
                    buf[..n].copy_from_slice(&remaining[..n]);
                    self.offset += n;
                    self.position += n as u64;
                    return Ok(n);
                }
            }
            if !self.advance() {
                return Ok(0);
            }
        }
    }
}

impl Seek for SegmentReader {
    /// Only position queries and forward skips are possible.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(_) => None,
        };
        let Some(target) = target.filter(|t| *t >= self.position) else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "segment stream cannot seek backwards",
            ));
        };
        let mut scratch = [0u8; 4096];
        while self.position < target {
            let want = ((target - self.position) as usize).min(scratch.len());
            if self.read(&mut scratch[..want])? == 0 {
                break;
            }
        }
        Ok(self.position)
    }
}

/// What the audio thread knows about one track's output that the wall clock
/// does not: how much of what it played was stand-in silence, and whether the
/// track has run out. Shared with the position clock (see `Position`).
#[derive(Clone, Default)]
pub(crate) struct Feed {
    starved_samples: Arc<AtomicU64>,
    ended: Arc<AtomicBool>,
    /// Samples per second across all channels; 0 for the empty default.
    samples_per_sec: u64,
}

impl Feed {
    /// Time spent playing silence because no decoded audio was ready.
    pub(crate) fn starved(&self) -> Duration {
        if self.samples_per_sec == 0 {
            return Duration::ZERO;
        }
        let samples = self.starved_samples.load(Ordering::Relaxed);
        Duration::from_nanos((samples as u128 * 1_000_000_000 / self.samples_per_sec as u128) as u64)
    }

    /// The decoder has finished and everything it produced has been played.
    pub(crate) fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }
}

/// Decoded PCM handed over in chunks from the decode thread. Emits silence
/// while starved (network stall) and ends once the sender is gone and the
/// buffer is drained.
pub(crate) struct PcmSource {
    rx: Receiver<Vec<f32>>,
    chunk: Vec<f32>,
    index: usize,
    channels: u16,
    sample_rate: u32,
    finished: bool,
    /// Silence samples emitted since the last whole frame. Audio only resumes
    /// when this is back to 0, or left and right would swap.
    off_frame: u16,
    feed: Feed,
}

impl PcmSource {
    pub(crate) fn new(rx: Receiver<Vec<f32>>, channels: u16, sample_rate: u32) -> Self {
        let feed = Feed {
            samples_per_sec: sample_rate as u64 * channels.max(1) as u64,
            ..Feed::default()
        };
        Self { rx, chunk: Vec::new(), index: 0, channels, sample_rate, finished: false, off_frame: 0, feed }
    }

    pub(crate) fn feed(&self) -> Feed {
        self.feed.clone()
    }

    fn silence(&mut self) -> f32 {
        self.off_frame = (self.off_frame + 1) % self.channels.max(1);
        self.feed.starved_samples.fetch_add(1, Ordering::Relaxed);
        0.0
    }
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if self.index < self.chunk.len() {
                if self.off_frame != 0 {
                    // Audio arrived part way through a frame of silence.
                    return Some(self.silence());
                }
                let s = self.chunk[self.index];
                self.index += 1;
                return Some(s);
            }
            if self.finished {
                self.feed.ended.store(true, Ordering::SeqCst);
                return None;
            }
            match self.rx.try_recv() {
                Ok(chunk) => {
                    self.chunk = chunk;
                    self.index = 0;
                }
                Err(TryRecvError::Empty) => return Some(self.silence()),
                Err(TryRecvError::Disconnected) => self.finished = true,
            }
        }
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn reader_stitches_chunks_and_ends_when_sender_drops() {
        let (tx, rx) = mpsc::channel();
        tx.send(Arc::new(b"abc".to_vec())).unwrap();
        tx.send(Arc::new(Vec::new())).unwrap(); // empty chunks are skipped
        tx.send(Arc::new(b"defgh".to_vec())).unwrap();
        drop(tx);
        let mut r = SegmentReader::new(rx);
        let mut out = String::new();
        r.read_to_string(&mut out).unwrap();
        assert_eq!(out, "abcdefgh");
        assert_eq!(r.seek(SeekFrom::Current(0)).unwrap(), 8);
    }

    #[test]
    fn reader_forward_seek_skips_bytes_and_backward_seek_fails() {
        let (tx, rx) = mpsc::channel();
        tx.send(Arc::new(b"0123456789".to_vec())).unwrap();
        drop(tx);
        let mut r = SegmentReader::new(rx);
        assert_eq!(r.seek(SeekFrom::Start(4)).unwrap(), 4);
        let mut two = [0u8; 2];
        r.read_exact(&mut two).unwrap();
        assert_eq!(&two, b"45");
        assert!(r.seek(SeekFrom::Start(1)).is_err());
        assert!(r.seek(SeekFrom::End(0)).is_err());
    }

    #[test]
    fn pcm_source_plays_chunks_pads_silence_then_ends() {
        let (tx, rx) = mpsc::sync_channel(4);
        let mut s = PcmSource::new(rx, 2, 44_100);
        tx.send(vec![0.5, -0.5]).unwrap();
        assert_eq!(s.next(), Some(0.5));
        assert_eq!(s.next(), Some(-0.5));
        assert_eq!(s.next(), Some(0.0), "starved: silence, not end");
        assert!(!s.feed().ended());
        tx.send(vec![0.25, 0.75]).unwrap();
        drop(tx);
        assert_eq!(s.next(), Some(0.0), "the silent frame is finished first");
        assert_eq!(s.next(), Some(0.25));
        assert_eq!(s.next(), Some(0.75));
        assert_eq!(s.next(), None);
        assert!(s.feed().ended(), "decoder gone and buffer drained");
        assert_eq!(s.channels(), 2);
        assert_eq!(s.sample_rate(), 44_100);
    }

    #[test]
    fn a_reader_being_probed_reports_end_of_stream_instead_of_waiting() {
        // The sender stays alive, as it does while the decoder is being built.
        let (tx, rx) = mpsc::channel();
        tx.send(Arc::new(b"not audio".to_vec())).unwrap();
        let mut r = SegmentReader::new(rx);
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"not audio");

        // Once the decoder is built, reads wait for the pump's next chunk.
        r.wait_for_more().store(true, Ordering::SeqCst);
        let feeder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            tx.send(Arc::new(b"more".to_vec())).unwrap();
        });
        let mut four = [0u8; 4];
        r.read_exact(&mut four).unwrap();
        assert_eq!(&four, b"more");
        feeder.join().unwrap();
    }

    #[test]
    fn audio_arriving_mid_frame_does_not_swap_left_and_right() {
        let (tx, rx) = mpsc::sync_channel(4);
        let mut s = PcmSource::new(rx, 2, 44_100);
        // Left is always positive, right always negative.
        tx.send(vec![0.1, -0.1]).unwrap();
        assert_eq!((s.next(), s.next()), (Some(0.1), Some(-0.1)));
        // Starved for the left sample of a frame; audio is back for the right.
        assert_eq!(s.next(), Some(0.0));
        tx.send(vec![0.2, -0.2, 0.3, -0.3]).unwrap();
        let rest: Vec<f32> = (0..5).map(|_| s.next().unwrap()).collect();
        assert_eq!(rest, [0.0, 0.2, -0.2, 0.3, -0.3]);
    }

    #[test]
    fn starvation_is_counted_as_time_not_played() {
        let (tx, rx) = mpsc::sync_channel(4);
        let mut s = PcmSource::new(rx, 2, 1_000);
        let feed = s.feed();
        tx.send(vec![0.5; 100]).unwrap();
        for _ in 0..100 {
            s.next();
        }
        assert_eq!(feed.starved(), Duration::ZERO, "real audio is not starvation");
        // Half a second of stereo at 1 kHz with nothing to play.
        for _ in 0..1_000 {
            assert_eq!(s.next(), Some(0.0));
        }
        assert_eq!(feed.starved(), Duration::from_millis(500));

        // The position clock leaves that half second out.
        let t0 = std::time::Instant::now();
        let pos = crate::player::Position {
            elapsed: Duration::from_secs(10),
            last_start: Some(t0),
            feed,
            ..Default::default()
        };
        assert_eq!(pos.at(t0 + Duration::from_secs(2)), Duration::from_millis(11_500));
        assert_eq!(Feed::default().starved(), Duration::ZERO);
    }
}

/// Decodes a real SoundCloud init + two segments through one decoder, the way
/// playback does. Needs fixtures (not committed); skips when absent:
/// `SCTUI_FIXTURE_DIR` containing init.mp4, seg0.m4s, seg1.m4s, durations.txt.
#[cfg(test)]
mod fixture_tests {
    use super::*;
    use rodio::Decoder;
    use std::sync::mpsc;

    #[test]
    fn continuous_decode_across_a_segment_boundary() {
        let Ok(dir) = std::env::var("SCTUI_FIXTURE_DIR") else { return };
        let read = |name: &str| std::fs::read(format!("{dir}/{name}")).unwrap();
        let durations: Vec<f64> = std::fs::read_to_string(format!("{dir}/durations.txt"))
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();

        let (tx, rx) = mpsc::channel();
        tx.send(Arc::new(read("init.mp4"))).unwrap();
        tx.send(Arc::new(read("seg0.m4s"))).unwrap();
        let decoder = Decoder::builder()
            .with_data(SegmentReader::new(rx))
            .with_seekable(false)
            .with_gapless(true)
            .build()
            .expect("probe init + first segment");
        let (channels, rate) = (decoder.channels() as usize, decoder.sample_rate() as usize);
        // Second segment arrives later, like the pump delivering it mid-playback.
        tx.send(Arc::new(read("seg1.m4s"))).unwrap();
        drop(tx);

        let samples: Vec<f32> = decoder.collect();
        let expected = (durations[0] + durations[1]) * rate as f64 * channels as f64;
        let got = samples.len() as f64;
        eprintln!("continuous decode: {got} samples, {channels}ch @ {rate} Hz, expected ~{expected:.0}");
        assert!(
            (got - expected).abs() / expected < 0.02,
            "decoded {got} samples, expected about {expected} for both segments"
        );
        // Nothing dropped out around the boundary: no run of digital silence there.
        let boundary = (durations[0] * rate as f64) as usize * channels;
        let window = &samples[boundary.saturating_sub(rate * channels / 10)..boundary + rate * channels / 10];
        let longest_zero_run = window
            .split(|s| s.abs() > 1e-4)
            .map(|z| z.len())
            .max()
            .unwrap_or(0);
        assert!(longest_zero_run < channels * 64, "gap of {longest_zero_run} zero samples at the seam");
    }
}
