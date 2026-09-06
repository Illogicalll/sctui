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
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

use rodio::Source;

/// Byte stream stitched from chunks arriving on a channel. Read-only forward
/// stream; the decoder is told it is not seekable.
pub(crate) struct SegmentReader {
    // Mutex only because rodio wants `Sync`; the decoder is the sole reader.
    rx: Mutex<Receiver<Arc<Vec<u8>>>>,
    current: Option<Arc<Vec<u8>>>,
    offset: usize,
    position: u64,
}

impl SegmentReader {
    pub(crate) fn new(rx: Receiver<Arc<Vec<u8>>>) -> Self {
        Self { rx: Mutex::new(rx), current: None, offset: 0, position: 0 }
    }

    /// Blocks until the next chunk arrives. `false` once the sender is gone.
    fn advance(&mut self) -> bool {
        match self.rx.lock().unwrap().recv() {
            Ok(chunk) => {
                self.current = Some(chunk);
                self.offset = 0;
                true
            }
            Err(_) => false,
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
}

impl PcmSource {
    pub(crate) fn new(rx: Receiver<Vec<f32>>, channels: u16, sample_rate: u32) -> Self {
        Self { rx, chunk: Vec::new(), index: 0, channels, sample_rate, finished: false }
    }
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if self.index < self.chunk.len() {
                let s = self.chunk[self.index];
                self.index += 1;
                return Some(s);
            }
            if self.finished {
                return None;
            }
            match self.rx.try_recv() {
                Ok(chunk) => {
                    self.chunk = chunk;
                    self.index = 0;
                }
                Err(TryRecvError::Empty) => return Some(0.0),
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
        tx.send(vec![0.25]).unwrap();
        drop(tx);
        assert_eq!(s.next(), Some(0.25));
        assert_eq!(s.next(), None);
        assert_eq!(s.channels(), 2);
        assert_eq!(s.sample_rate(), 44_100);
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
