#[derive(Clone)]
pub struct SinSignal {
    x: f64,
    interval: f64,
    period: f64,
    scale: f64,
}

impl SinSignal {
    pub const fn new(interval: f64, period: f64, scale: f64) -> Self {
        Self {
            x: 0.0,
            interval,
            period,
            scale,
        }
    }
}

impl Iterator for SinSignal {
    type Item = (f64, f64);
    fn next(&mut self) -> Option<Self::Item> {
        let point = (self.x, (self.x * 1.0 / self.period).sin() * self.scale);
        self.x += self.interval;
        Some(point)
    }
}

pub fn on_tick(data: &mut Vec<(f64, f64)>, window: &mut [f64; 2], signal: &mut SinSignal) {
    data.drain(0..10);
    data.extend(signal.by_ref().take(10));
    window[0] += 1.0;
    window[1] += 1.0;
}
