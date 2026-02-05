use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::api::Track;
use crate::player::stream::hls::HlsManifest;

pub(crate) const SEGMENT_CACHE_CAP: usize = 12;
pub(crate) const HLS_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 60);

#[derive(Debug)]
pub(crate) struct SegmentCache {
    cap: usize,
    order: VecDeque<usize>,
    map: HashMap<usize, Arc<Vec<u8>>>,
}

impl SegmentCache {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }

    pub(crate) fn get(&mut self, idx: usize) -> Option<Arc<Vec<u8>>> {
        let bytes = self.map.get(&idx).cloned();
        if bytes.is_some() {
            self.order.retain(|&i| i != idx);
            self.order.push_back(idx);
        }
        bytes
    }

    pub(crate) fn insert(&mut self, idx: usize, bytes: Arc<Vec<u8>>) {
        if self.map.contains_key(&idx) {
            self.order.retain(|&i| i != idx);
        }
        self.order.push_back(idx);
        self.map.insert(idx, bytes);

        while self.order.len() > self.cap {
            if let Some(evict) = self.order.pop_front() {
                self.map.remove(&evict);
            }
        }
    }
}

pub(crate) struct CachedHls {
    pub track_urn: String,
    pub fetched_at: Instant,
    pub manifest: Arc<HlsManifest>,
    pub init_bytes: Arc<Vec<u8>>,
    pub segment_cache: Arc<Mutex<SegmentCache>>,
}

impl CachedHls {
    pub(crate) fn is_valid_for(&self, track: &Track, now: Instant) -> bool {
        self.track_urn == track.track_urn && now.duration_since(self.fetched_at) < HLS_CACHE_TTL
    }
}
