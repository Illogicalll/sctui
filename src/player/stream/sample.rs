use dasp_sample::ToSample;
use rodio::Source;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub(crate) const WAVE_BUFFER_CAP: usize = 2048;

pub(crate) struct TapSource<S> {
    inner: S,
    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl<S> TapSource<S> {
    pub(crate) fn new(inner: S, buffer: Arc<Mutex<VecDeque<f32>>>) -> Self {
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

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}
