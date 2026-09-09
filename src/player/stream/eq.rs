//! Six-band peaking equaliser, wrapped around the PCM on its way into the sink.
//!
//! The gains live in process globals rather than being threaded down from the
//! TUI. That is what makes the preview live: the decode thread runs ~1.5 s of
//! look-ahead, so anything applied before the channel would only be heard a
//! second and a half after the key press, whereas [`EqSource`] sits after it and
//! re-reads the gains as the audio callback pulls each sample.
//!
//! ponytail: one equaliser per process, which is all a single-sink player needs.
//! If sctui ever drives two output streams, move these statics onto
//! `PlaybackEngine` and hand the source an `Arc` to them instead.

use std::sync::atomic::{AtomicI8, AtomicU64, Ordering};

use rodio::Source;

/// Band centre frequencies, in Hz.
pub(crate) const BANDS: [f32; 6] = [60.0, 230.0, 910.0, 3000.0, 8000.0, 16000.0];

/// How far either side of flat one band can be pushed, in dB.
pub(crate) const MAX_GAIN_DB: i8 = 12;

/// Bandwidth of each peaking filter. The centres sit one to two octaves apart,
/// so a Q around 1 leaves neighbouring bands overlapping rather than notched.
const Q: f32 = 1.0;

static GAINS: [AtomicI8; BANDS.len()] = [const { AtomicI8::new(0) }; BANDS.len()];
/// Bumped by [`set`]; a live [`EqSource`] recomputes its coefficients when it moves.
static VERSION: AtomicU64 = AtomicU64::new(0);

/// Install new gains. Takes effect on whatever is already playing.
pub(crate) fn set(gains: [i8; BANDS.len()]) {
    for (slot, db) in GAINS.iter().zip(gains) {
        slot.store(db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB), Ordering::Relaxed);
    }
    VERSION.fetch_add(1, Ordering::Release);
}

/// Human label for a band, e.g. `60 Hz` or `16 kHz`.
pub(crate) fn band_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// Signed label for a gain, e.g. `+3 dB`, `-12 dB`, `0 dB`.
pub(crate) fn gain_label(db: i8) -> String {
    if db == 0 {
        "0 dB".to_string()
    } else {
        format!("{db:+} dB")
    }
}

/// One RBJ-cookbook peaking filter, already normalised by `a0`.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    /// False for a flat band (or one above Nyquist), which is then skipped
    /// entirely so a default EQ is bit-exact passthrough.
    active: bool,
}

impl Biquad {
    fn peaking(freq: f32, sample_rate: f32, gain_db: f32) -> Self {
        // A band at or above Nyquist has no coefficients worth computing.
        if gain_db == 0.0 || freq * 2.0 >= sample_rate {
            return Self::default();
        }
        let amp = 10f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * Q);
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha / amp;
        Self {
            b0: (1.0 + alpha * amp) / a0,
            b1: (-2.0 * cos_w0) / a0,
            b2: (1.0 - alpha * amp) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha / amp) / a0,
            active: true,
        }
    }
}

/// The two-sample history one biquad needs, for one channel.
#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    fn step(&mut self, c: &Biquad, x: f32) -> f32 {
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Runs the six bands over an interleaved source, one filter chain per channel.
pub(crate) struct EqSource<S> {
    inner: S,
    /// The [`VERSION`] the current coefficients were built from.
    version: u64,
    coeffs: [Biquad; BANDS.len()],
    states: Vec<[BiquadState; BANDS.len()]>,
    /// Which channel the next sample belongs to; samples arrive interleaved.
    channel: usize,
    any_active: bool,
}

impl<S: Source> EqSource<S> {
    pub(crate) fn new(inner: S) -> Self {
        let channels = inner.channels().max(1) as usize;
        let mut source = Self {
            inner,
            version: 0,
            coeffs: [Biquad::default(); BANDS.len()],
            states: vec![[BiquadState::default(); BANDS.len()]; channels],
            channel: 0,
            any_active: false,
        };
        source.reload();
        source
    }

    /// Recompute coefficients from the globals, leaving the filter histories
    /// alone so a mid-track change glides rather than clicks.
    fn reload(&mut self) {
        // Version first: reading gains that are newer than it only costs one
        // redundant reload next sample, whereas the other order could miss a change.
        self.version = VERSION.load(Ordering::Acquire);
        let sample_rate = self.inner.sample_rate() as f32;
        self.coeffs = std::array::from_fn(|i| {
            Biquad::peaking(BANDS[i], sample_rate, f32::from(GAINS[i].load(Ordering::Relaxed)))
        });
        self.any_active = self.coeffs.iter().any(|c| c.active);
    }
}

impl<S: Source> Iterator for EqSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        if self.version != VERSION.load(Ordering::Acquire) {
            self.reload();
        }
        let channel = self.channel;
        self.channel = (channel + 1) % self.states.len();
        if !self.any_active {
            return Some(sample);
        }
        let mut out = sample;
        for (coeff, state) in self.coeffs.iter().zip(self.states[channel].iter_mut()) {
            if coeff.active {
                out = state.step(coeff, out);
            }
        }
        // Boosted bands can push past full scale; clip rather than wrap.
        Some(out.clamp(-1.0, 1.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S: Source> Source for EqSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;
    /// Well under full scale, so a +12 dB boost has room before the clamp.
    const AMPLITUDE: f32 = 0.2;

    struct Tone {
        samples: std::vec::IntoIter<f32>,
        channels: u16,
    }

    impl Iterator for Tone {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            self.samples.next()
        }
    }

    impl Source for Tone {
        fn current_span_len(&self) -> Option<usize> {
            None
        }
        fn channels(&self) -> u16 {
            self.channels
        }
        fn sample_rate(&self) -> u32 {
            RATE
        }
        fn total_duration(&self) -> Option<std::time::Duration> {
            None
        }
    }

    /// One second of a sine. `right` is the amplitude of the second channel.
    fn tone(freq: f32, right: Option<f32>) -> Tone {
        let channels = if right.is_some() { 2 } else { 1 };
        let samples = (0..RATE as usize)
            .flat_map(|i| {
                let phase = (std::f32::consts::TAU * freq * i as f32 / RATE as f32).sin();
                [Some(AMPLITUDE * phase), right.map(|r| r * phase)]
            })
            .flatten()
            .collect::<Vec<_>>();
        Tone { samples: samples.into_iter(), channels }
    }

    /// RMS of the filtered tone, skipping the first 100 ms of filter start-up.
    fn rms(freq: f32) -> f32 {
        let out: Vec<f32> = EqSource::new(tone(freq, None)).collect();
        let tail = &out[RATE as usize / 10..];
        (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt()
    }

    // One test, because the gains are global and parallel tests would race.
    #[test]
    fn bands_boost_their_own_frequency_and_keep_channels_apart() {
        let flat = AMPLITUDE / 2f32.sqrt();

        set([0; BANDS.len()]);
        let passthrough: Vec<f32> = EqSource::new(tone(60.0, None)).collect();
        let untouched: Vec<f32> = tone(60.0, None).collect();
        assert_eq!(passthrough, untouched, "a flat EQ must not touch the samples");

        // +12 dB on the 60 Hz band is a 10^(12/20) = ~3.98x voltage gain at the centre.
        set([MAX_GAIN_DB, 0, 0, 0, 0, 0]);
        let boosted = rms(BANDS[0]);
        assert!(
            (boosted / flat - 3.98).abs() < 0.15,
            "60 Hz should be lifted ~4x, got {boosted} vs flat {flat}"
        );
        let distant = rms(BANDS[4]);
        assert!(
            (distant / flat - 1.0).abs() < 0.05,
            "8 kHz should be untouched by the 60 Hz band, got {distant} vs flat {flat}"
        );

        // A cut is the mirror image.
        set([-MAX_GAIN_DB, 0, 0, 0, 0, 0]);
        let cut = rms(BANDS[0]);
        assert!((cut / flat - 0.251).abs() < 0.05, "60 Hz should be cut ~4x, got {cut}");

        // Silence on the right channel must stay silent: each channel keeps its
        // own filter history, so nothing bleeds across the interleave.
        set([MAX_GAIN_DB; BANDS.len()]);
        let stereo: Vec<f32> = EqSource::new(tone(BANDS[0], Some(0.0))).collect();
        assert!(
            stereo.iter().skip(1).step_by(2).all(|s| *s == 0.0),
            "the silent channel picked up audio from the other one"
        );

        // Gains outside the range are clamped, not wrapped.
        set([100; BANDS.len()]);
        assert!(GAINS.iter().all(|g| g.load(Ordering::Relaxed) == MAX_GAIN_DB));
        set([0; BANDS.len()]);
    }

    #[test]
    fn labels_switch_to_kilohertz() {
        assert_eq!(band_label(60.0), "60 Hz");
        assert_eq!(band_label(910.0), "910 Hz");
        assert_eq!(band_label(3000.0), "3 kHz");
        assert_eq!(band_label(16000.0), "16 kHz");
        assert_eq!(gain_label(0), "0 dB");
        assert_eq!(gain_label(3), "+3 dB");
        assert_eq!(gain_label(-12), "-12 dB");
    }
}
