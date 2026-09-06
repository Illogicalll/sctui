use std::sync::mpsc::Sender;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use crate::player::Position;
use crate::player::stream::cache::SegmentCache;
use crate::player::stream::hls::HlsManifest;

pub(crate) const PREFETCH_SEGMENTS: usize = 3;

pub(crate) struct SegmentPumpParams {
    pub client: reqwest::blocking::Client,
    pub generation: Arc<AtomicU64>,
    pub generation_value: u64,
    pub manifest: Arc<HlsManifest>,
    pub segment_cache: Arc<Mutex<SegmentCache>>,
    pub start_segment_index: usize,
    /// Feeds the track's single continuous decoder (see `stream::reader`).
    pub bytes_tx: Sender<Arc<Vec<u8>>>,
    pub is_playing_flag: Arc<std::sync::atomic::AtomicBool>,
    pub position: Arc<Mutex<Position>>,
}

pub(crate) fn spawn_segment_pump(params: SegmentPumpParams) {
    std::thread::spawn(move || {
        let SegmentPumpParams {
            client,
            generation,
            generation_value,
            manifest,
            segment_cache,
            start_segment_index,
            bytes_tx,
            is_playing_flag,
            position,
        } = params;

        let mut next_index = start_segment_index.saturating_add(1);
        while next_index < manifest.segments.len() {
            if generation.load(Ordering::SeqCst) != generation_value {
                break;
            }

            let approx_pos_ms = {
                let pos = position.lock().unwrap();
                let base = pos.elapsed;
                if is_playing_flag.load(Ordering::SeqCst) {
                    if let Some(start) = pos.last_start {
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

            if generation.load(Ordering::SeqCst) != generation_value {
                break;
            }
            // Receiver gone means the track was stopped or replaced.
            if bytes_tx.send(media_bytes).is_err() {
                break;
            }

            next_index += 1;
        }
    });
}
