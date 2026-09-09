use rodio::Source;
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

pub(crate) const WAVE_BUFFER_CAP: usize = 8192;

pub(crate) struct TapSource<S> {
    inner: S,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    /// The visualizer follows one track at a time. A track crossfading out is
    /// still audible but no longer the current generation, and two tracks
    /// pushing samples into the same buffer interleave into noise.
    generation: Arc<AtomicU64>,
    generation_value: u64,
}

impl<S> TapSource<S> {
    pub(crate) fn new(
        inner: S,
        buffer: Arc<Mutex<VecDeque<f32>>>,
        generation: Arc<AtomicU64>,
        generation_value: u64,
    ) -> Self {
        Self { inner, buffer, generation, generation_value }
    }
}

impl<S: Source> Iterator for TapSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        if self.generation.load(Ordering::SeqCst) == self.generation_value {
            let mut buffer = self.buffer.lock().unwrap();
            if buffer.len() >= WAVE_BUFFER_CAP {
                buffer.pop_front();
            }
            buffer.push_back(sample);
        }
        Some(sample)
    }
}

impl<S: Source> Source for TapSource<S> {
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
    use rodio::buffer::SamplesBuffer;

    #[test]
    fn a_superseded_track_keeps_playing_but_stops_feeding_the_visualizer() {
        let buffer = Arc::new(Mutex::new(VecDeque::new()));
        let generation = Arc::new(AtomicU64::new(1));
        let mut tap = TapSource::new(
            SamplesBuffer::new(1, 44_100, vec![0.1, 0.2, 0.3, 0.4]),
            Arc::clone(&buffer),
            Arc::clone(&generation),
            1,
        );

        assert_eq!(tap.next(), Some(0.1));
        assert_eq!(tap.next(), Some(0.2));
        // The next track takes over; this one is now crossfading out.
        generation.store(2, Ordering::SeqCst);
        assert_eq!(tap.next(), Some(0.3), "still audible");
        assert_eq!(tap.next(), Some(0.4));

        let tapped: Vec<f32> = buffer.lock().unwrap().iter().copied().collect();
        assert_eq!(tapped, vec![0.1, 0.2]);
    }
}
