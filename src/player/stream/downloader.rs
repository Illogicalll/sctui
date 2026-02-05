use std::io::Cursor;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use rodio::{Decoder, Sink};

use crate::player::stream::cache::SegmentCache;
use crate::player::stream::hls::HlsManifest;
use crate::player::stream::sample::TapSource;

pub(crate) const PREFETCH_SEGMENTS: usize = 3;

pub(crate) struct SegmentPumpParams {
    pub client: reqwest::blocking::Client,
    pub generation: Arc<AtomicU64>,
    pub generation_value: u64,
    pub manifest: Arc<HlsManifest>,
    pub init_bytes: Arc<Vec<u8>>,
    pub segment_cache: Arc<Mutex<SegmentCache>>,
    pub start_segment_index: usize,
    pub sink_arc: Arc<Mutex<Option<Sink>>>,
    pub wave_buffer: Arc<Mutex<std::collections::VecDeque<f32>>>,
    pub is_playing_flag: Arc<std::sync::atomic::AtomicBool>,
    pub elapsed_time: Arc<Mutex<Duration>>,
    pub last_start: Arc<Mutex<Option<std::time::Instant>>>,
}

pub(crate) fn spawn_segment_pump(params: SegmentPumpParams) {
    std::thread::spawn(move || {
        let SegmentPumpParams {
            client,
            generation,
            generation_value,
            manifest,
            init_bytes,
            segment_cache,
            start_segment_index,
            sink_arc,
            wave_buffer,
            is_playing_flag,
            elapsed_time,
            last_start,
        } = params;

        let mut next_index = start_segment_index.saturating_add(1);
        while next_index < manifest.segments.len() {
            if generation.load(Ordering::SeqCst) != generation_value {
                break;
            }

            let approx_pos_ms = {
                let base = *elapsed_time.lock().unwrap();
                if is_playing_flag.load(Ordering::SeqCst) {
                    if let Some(start) = *last_start.lock().unwrap() {
                        (base + start.elapsed()).as_millis() as u64
                    } else {
                        base.as_millis() as u64
                    }
                } else {
                    base.as_millis() as u64
                }
            };
            let (current_seg, _) = manifest.locate_position(approx_pos_ms);
            if next_index > current_seg.saturating_add(PREFETCH_SEGMENTS) {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }

            let media_bytes = {
                let mut cache_guard = segment_cache.lock().unwrap();
                if let Some(bytes) = cache_guard.get(next_index) {
                    bytes
                } else {
                    drop(cache_guard);
                    let url = &manifest.segments[next_index].url;
                    let bytes = match client.get(url.as_str()).send() {
                        Ok(resp) => match resp.error_for_status() {
                            Ok(ok) => match ok.bytes() {
                                Ok(b) => b.to_vec(),
                                Err(_) => break,
                            },
                            Err(_) => break,
                        },
                        Err(_) => break,
                    };
                    let arc = Arc::new(bytes);
                    let mut cache_guard = segment_cache.lock().unwrap();
                    cache_guard.insert(next_index, Arc::clone(&arc));
                    arc
                }
            };

            let combined = combine_init_and_segment(&init_bytes, &media_bytes);
            let decoder = match Decoder::new(Cursor::new(combined)) {
                Ok(d) => d,
                Err(_) => break,
            };

            let tapped = TapSource::new(decoder, Arc::clone(&wave_buffer));
            if generation.load(Ordering::SeqCst) != generation_value {
                break;
            }
            let guard = sink_arc.lock().unwrap();
            if generation.load(Ordering::SeqCst) != generation_value {
                break;
            }
            if let Some(ref sink) = *guard {
                sink.append(tapped);
            } else {
                break;
            }

            next_index += 1;
        }
    });
}

fn combine_init_and_segment(init_bytes: &[u8], segment_bytes: &[u8]) -> Vec<u8> {
    let mut combined = Vec::with_capacity(init_bytes.len() + segment_bytes.len());
    combined.extend_from_slice(init_bytes);
    combined.extend_from_slice(segment_bytes);
    combined
}
