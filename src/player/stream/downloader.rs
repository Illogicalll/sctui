use reqwest::Url;
use std::sync::mpsc::Sender;
use std::sync::{
    Arc, Mutex,
    atomic::AtomicU64,
};
use std::time::Duration;

use crate::api::rate_limit::{self, SendGated};
use crate::auth::Token;
use crate::player::Position;
use crate::player::stream::cache::SegmentCache;
use crate::player::stream::engine::is_live;
use crate::player::stream::hls::{HlsManifest, resolve_manifest};

pub(crate) const PREFETCH_SEGMENTS: usize = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);
/// Failed fetches of one segment before the manifest is assumed stale rather
/// than the network flaky.
const ATTEMPTS_BEFORE_REFRESH: usize = 3;
/// Fresh signed URLs that may still fail before the track is given up on.
const MAX_REFRESHES: usize = 2;

pub(crate) struct SegmentPumpParams {
    pub client: reqwest::blocking::Client,
    pub generation: Arc<AtomicU64>,
    /// The one older generation still allowed to run: the track fading out
    /// behind this one. See `engine::is_live`.
    pub fading: Arc<AtomicU64>,
    pub generation_value: u64,
    pub manifest: Arc<HlsManifest>,
    pub segment_cache: Arc<Mutex<SegmentCache>>,
    pub start_segment_index: usize,
    /// Feeds the track's single continuous decoder (see `stream::reader`).
    pub bytes_tx: Sender<Arc<Vec<u8>>>,
    pub position: Arc<Mutex<Position>>,
    /// Needed to re-resolve the manifest when its signed URLs expire.
    pub track_urn: String,
    pub token: Arc<Mutex<Token>>,
}

/// Downloads segment `index`, replacing `manifest` with a freshly resolved one
/// when the URLs in hand stop working.
///
/// SoundCloud's segment URLs are signed and expire, and the manifest is fetched
/// once when the track starts, so anything that leaves the pump idle for long
/// enough — a long pause, a suspended laptop, a long track — outlives the
/// signature. A plain retry loop would then hammer a permanently dead URL, so
/// after `ATTEMPTS_BEFORE_REFRESH` failures the manifest is re-resolved and the
/// same segment retried against the new URL.
///
/// A refresh that itself fails means the network is down rather than the URL
/// being stale, so it costs nothing from the refresh budget and the loop keeps
/// waiting — a dropped connection still recovers on its own. Returns `None`
/// only when there is nothing left to try: cancelled, out of refreshes, or the
/// track came back re-segmented (which would misalign the cached indices).
///
/// A rate limit is neither of those: retrying is exactly what SoundCloud has
/// asked not to happen, and a fresh manifest is one more request into the same
/// limit. While `limited` says one is in force nothing is fetched or refreshed
/// and nothing counts as a failure; the segment is asked for again once it lifts.
fn fetch_segment(
    manifest: &mut Arc<HlsManifest>,
    index: usize,
    retry_delay: Duration,
    fetch: &mut impl FnMut(&Url) -> anyhow::Result<Vec<u8>>,
    refresh: &mut impl FnMut() -> anyhow::Result<HlsManifest>,
    cancelled: &mut impl FnMut() -> bool,
    limited: &mut impl FnMut() -> bool,
) -> Option<Vec<u8>> {
    let mut failures = 0usize;
    let mut refreshes = 0usize;

    loop {
        if cancelled() {
            return None;
        }
        if limited() {
            std::thread::sleep(retry_delay);
            continue;
        }
        match fetch(&manifest.segments[index].url) {
            Ok(bytes) => return Some(bytes),
            // That was the 429 itself.
            Err(_) if limited() => continue,
            Err(_) => failures += 1,
        }

        if failures >= ATTEMPTS_BEFORE_REFRESH
            && let Ok(fresh) = refresh()
        {
            if fresh.segment_start_ms != manifest.segment_start_ms {
                return None;
            }
            refreshes += 1;
            if refreshes > MAX_REFRESHES {
                return None;
            }
            *manifest = Arc::new(fresh);
            failures = 0;
        }

        std::thread::sleep(retry_delay);
    }
}

pub(crate) fn spawn_segment_pump(params: SegmentPumpParams) {
    std::thread::spawn(move || {
        let SegmentPumpParams {
            client,
            generation,
            fading,
            generation_value,
            mut manifest,
            segment_cache,
            start_segment_index,
            bytes_tx,
            position,
            track_urn,
            token,
        } = params;

        let is_cancelled = || !is_live(&generation, &fading, generation_value);

        let mut next_index = start_segment_index.saturating_add(1);
        while next_index < manifest.segments.len() {
            if is_cancelled() {
                break;
            }

            let approx_pos_ms = position.lock().unwrap().now().as_millis() as u64;
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
                    let bytes = fetch_segment(
                        &mut manifest,
                        next_index,
                        RETRY_DELAY,
                        &mut |url| Ok(client.get(url.as_str()).send_gated()?.bytes()?.to_vec()),
                        &mut || resolve_manifest(&client, &track_urn, &token),
                        &mut || is_cancelled(),
                        &mut || rate_limit::until().is_some(),
                    );
                    let Some(bytes) = bytes else { break };
                    let arc = Arc::new(bytes);
                    let mut cache_guard = segment_cache.lock().unwrap();
                    cache_guard.insert(next_index, Arc::clone(&arc));
                    arc
                }
            };

            if is_cancelled() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(tag: &str, segments: usize) -> HlsManifest {
        HlsManifest {
            init_url: None,
            segments: (0..segments)
                .map(|i| crate::player::stream::hls::HlsSegment {
                    url: Url::parse(&format!("https://cdn.test/{tag}/{i}?sig={tag}")).unwrap(),
                })
                .collect(),
            segment_start_ms: (0..segments as u64).map(|i| i * 10_000).collect(),
            total_duration_ms: segments as u64 * 10_000,
        }
    }

    /// `fetch` succeeds only for URLs signed by the manifest `refresh` last handed out.
    fn expiring_fetcher(valid: Arc<Mutex<String>>) -> impl FnMut(&Url) -> anyhow::Result<Vec<u8>> {
        move |url: &Url| {
            let sig = valid.lock().unwrap().clone();
            if url.query() == Some(format!("sig={sig}").as_str()) {
                Ok(b"segment".to_vec())
            } else {
                Err(anyhow::anyhow!("403 expired"))
            }
        }
    }

    #[test]
    fn expired_urls_are_refreshed_and_the_same_segment_still_arrives() {
        let mut m = Arc::new(manifest("stale", 4));
        let valid = Arc::new(Mutex::new("fresh".to_string()));
        let mut refreshes = 0;

        let bytes = fetch_segment(
            &mut m,
            2,
            Duration::ZERO,
            &mut expiring_fetcher(Arc::clone(&valid)),
            &mut || {
                refreshes += 1;
                Ok(manifest("fresh", 4))
            },
            &mut || false,
            &mut || false,
        );

        assert_eq!(bytes.as_deref(), Some(&b"segment"[..]));
        assert_eq!(refreshes, 1, "one refresh should be enough");
        assert_eq!(m.segments[2].url.query(), Some("sig=fresh"));
    }

    #[test]
    fn a_permanently_dead_segment_gives_up_instead_of_retrying_forever() {
        let mut m = Arc::new(manifest("stale", 4));
        let mut refreshes = 0;

        let bytes = fetch_segment(
            &mut m,
            0,
            Duration::ZERO,
            &mut |_| Err(anyhow::anyhow!("404")),
            &mut || {
                refreshes += 1;
                Ok(manifest("fresh", 4))
            },
            &mut || false,
            &mut || false,
        );

        assert!(bytes.is_none());
        assert_eq!(refreshes, MAX_REFRESHES + 1);
    }

    #[test]
    fn a_refresh_that_fails_is_free_so_a_network_outage_still_recovers() {
        let mut m = Arc::new(manifest("stale", 4));
        let valid = Arc::new(Mutex::new("nothing-works-yet".to_string()));
        let mut fetch = expiring_fetcher(Arc::clone(&valid));
        // Offline for a while: every fetch and every refresh fails. Then the
        // network returns and the original URLs turn out to be fine after all.
        let refresh_attempts = std::cell::Cell::new(0);
        let bytes = fetch_segment(
            &mut m,
            1,
            Duration::ZERO,
            &mut |url| {
                let out = fetch(url);
                if refresh_attempts.get() >= 5 {
                    *valid.lock().unwrap() = "stale".to_string();
                }
                out
            },
            &mut || {
                refresh_attempts.set(refresh_attempts.get() + 1);
                Err(anyhow::anyhow!("dns failure"))
            },
            &mut || false,
            &mut || false,
        );

        assert_eq!(bytes.as_deref(), Some(&b"segment"[..]));
        assert!(refresh_attempts.get() >= 5, "kept waiting rather than giving up");
        assert_eq!(m.segments[1].url.query(), Some("sig=stale"), "manifest untouched");
    }

    #[test]
    fn a_re_segmented_track_is_given_up_on_rather_than_stitched_wrongly() {
        let mut m = Arc::new(manifest("stale", 4));
        let bytes = fetch_segment(
            &mut m,
            0,
            Duration::ZERO,
            &mut |_| Err(anyhow::anyhow!("403 expired")),
            &mut || Ok(manifest("fresh", 7)),
            &mut || false,
            &mut || false,
        );
        assert!(bytes.is_none());
        assert_eq!(m.segments.len(), 4, "cached segment indices stay as they were");
    }

    /// #55: a 429 used to be retried twice a second for ever, since the manifest
    /// refresh failed with a 429 of its own and a failed refresh is free.
    #[test]
    fn a_rate_limit_is_waited_out_not_retried_or_refreshed() {
        let mut m = Arc::new(manifest("signed", 4));
        // The first fetch is answered 429; the limit lifts 20 checks later.
        let limited_for = std::cell::Cell::new(0u32);
        let mut fetches = 0;
        let mut refreshes = 0;

        let bytes = fetch_segment(
            &mut m,
            1,
            Duration::ZERO,
            &mut |_| {
                fetches += 1;
                if fetches == 1 {
                    limited_for.set(20);
                    return Err(anyhow::anyhow!("429"));
                }
                Ok(b"segment".to_vec())
            },
            &mut || {
                refreshes += 1;
                Err(anyhow::anyhow!("429"))
            },
            &mut || false,
            &mut || {
                limited_for.set(limited_for.get().saturating_sub(1));
                limited_for.get() > 0
            },
        );

        assert_eq!(bytes.as_deref(), Some(&b"segment"[..]), "the track carries on once the limit lifts");
        assert_eq!(fetches, 2, "the 429 and the one after the limit lifted; none in between");
        assert_eq!(refreshes, 0, "a fresh manifest would be one more request into the limit");
    }

    #[test]
    fn cancellation_stops_the_retry_loop() {
        let mut m = Arc::new(manifest("stale", 4));
        let mut ticks = 0;
        let bytes = fetch_segment(
            &mut m,
            0,
            Duration::ZERO,
            &mut |_| Err(anyhow::anyhow!("403 expired")),
            &mut || Ok(manifest("stale", 4)),
            &mut || {
                ticks += 1;
                ticks > 2
            },
            &mut || false,
        );
        assert!(bytes.is_none());
    }
}
