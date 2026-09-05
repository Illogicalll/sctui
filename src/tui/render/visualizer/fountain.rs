use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use rand::Rng;
use ratatui::{
    Frame,
    layout::Rect,
    symbols::Marker,
    widgets::canvas::{Canvas, Points},
};

use super::common::{dot_bounds, frame_block, gradient_cyan_magenta, spectrum_bands, tick_dt};

const BANDS: usize = 16;
const MAX_PARTICLES: usize = 1500;
const GRAVITY: f32 = 60.0; // dots / s²
const IDLE_RATE: f32 = 30.0; // particles / s with no bass at all
const BASS_RATE: f32 = 900.0; // extra particles / s at full bass
const SLABS: usize = 6;

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[derive(Default)]
struct FountainState {
    particles: Vec<Particle>,
    last: Option<Instant>,
    spawn_carry: f32,
}

static STATE: OnceLock<Mutex<FountainState>> = OnceLock::new();

/// Particles launched from the bottom centre. Bass sets launch speed and
/// count, mids widen the spray. Colour cools from magenta to cyan with height.
pub fn render_particle_fountain(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    let (w, h) = dot_bounds(inner);
    if w < 8.0 || h < 8.0 {
        frame.render_widget(block, area);
        return;
    }
    let (wf, hf) = (w as f32, h as f32);

    let bands = spectrum_bands(samples, BANDS);
    let bass = (bands[0..3].iter().sum::<f32>() / 3.0).clamp(0.0, 1.0);
    let mids = (bands[3..9].iter().sum::<f32>() / 6.0).clamp(0.0, 1.0);

    let mut state = STATE.get_or_init(|| Mutex::new(FountainState::default())).lock().unwrap();
    let dt = tick_dt(&mut state.last);
    let mut rng = rand::thread_rng();

    // Spawn. Carry the fractional remainder so low rates still emit.
    state.spawn_carry += (IDLE_RATE + BASS_RATE * bass) * dt;
    let count = state.spawn_carry.floor() as usize;
    state.spawn_carry -= count as f32;
    let peak_height = hf * (0.30 + 0.65 * bass);
    let speed = (2.0 * GRAVITY * peak_height).sqrt();
    let spread = 0.18 + 0.55 * mids;
    for _ in 0..count {
        let theta = rng.gen_range(-spread..spread);
        let s = speed * rng.gen_range(0.85..1.05);
        state.particles.push(Particle {
            x: wf / 2.0 + rng.gen_range(-1.0..1.0) * wf * 0.015,
            y: 0.0,
            vx: s * theta.sin(),
            vy: s * theta.cos(),
        });
    }
    if state.particles.len() > MAX_PARTICLES {
        let excess = state.particles.len() - MAX_PARTICLES;
        state.particles.drain(..excess);
    }

    // Integrate.
    for p in &mut state.particles {
        p.vy -= GRAVITY * dt;
        p.x += p.vx * dt;
        p.y += p.vy * dt;
    }
    state.particles.retain(|p| p.y >= 0.0 && p.x >= 0.0 && p.x < wf);

    // Bucket by height so each slab gets its own colour.
    let mut slabs: Vec<Vec<(f64, f64)>> = vec![Vec::new(); SLABS];
    for p in &state.particles {
        let s = ((p.y / hf) * SLABS as f32).floor().clamp(0.0, (SLABS - 1) as f32) as usize;
        slabs[s].push((p.x as f64, p.y as f64));
    }
    drop(state);

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(|ctx| {
            for (s, coords) in slabs.iter().enumerate() {
                if coords.is_empty() {
                    continue;
                }
                let t = 1.0 - s as f32 / (SLABS - 1) as f32; // bottom hot, top cool
                ctx.draw(&Points { coords, color: gradient_cyan_magenta(t) });
            }
        });
    frame.render_widget(canvas, area);
}
