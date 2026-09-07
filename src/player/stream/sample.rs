use rodio::Source;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub(crate) const WAVE_BUFFER_CAP: usize = 8192;

pub(crate) struct TapSource<S> {
    inner: S,
    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl<S> TapSource<S> {
    pub(crate) fn new(inner: S, buffer: Arc<Mutex<VecDeque<f32>>>) -> Self {
        Self { inner, buffer }
    }
}

impl<S: Source> Iterator for TapSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.len() >= WAVE_BUFFER_CAP {
            buffer.pop_front();
        }
        buffer.push_back(sample);
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
