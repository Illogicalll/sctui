use anyhow::Context;
use m3u8_rs::Playlist;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub(crate) struct StreamsResponse {
    pub hls_aac_160_url: Option<String>,
    #[serde(rename = "hls_aac_96_url")]
    pub hls_aac_96_url: Option<String>,
    pub hls_mp3_128_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct HlsSegment {
    pub url: Url,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct HlsManifest {
    pub init_url: Option<Url>,
    pub segments: Vec<HlsSegment>,
    pub segment_start_ms: Vec<u64>,
    pub total_duration_ms: u64,
}

impl HlsManifest {
    pub(crate) fn locate_position(&self, position_ms: u64) -> (usize, u64) {
        if self.segments.is_empty() {
            return (0, 0);
        }

        let clamped = position_ms.min(self.total_duration_ms.saturating_sub(1));

        let idx = match self.segment_start_ms.binary_search(&clamped) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };

        let offset_ms = clamped.saturating_sub(self.segment_start_ms[idx]);
        (idx, offset_ms)
    }

    pub(crate) fn fetch(
        client: &reqwest::blocking::Client,
        playlist_url: &Url,
        access_token: &str,
    ) -> anyhow::Result<Self> {
        let mut url = playlist_url.clone();
        for _ in 0..5 {
            let playlist_content = client
                .get(url.as_str())
                .bearer_auth(access_token)
                .send()
                .with_context(|| format!("failed to fetch playlist {}", url))?
                .error_for_status()
                .with_context(|| format!("playlist returned error status {}", url))?
                .bytes()
                .with_context(|| format!("failed to read playlist bytes {}", url))?;

            let parsed = m3u8_rs::parse_playlist_res(&playlist_content);
            match parsed {
                Ok(Playlist::MediaPlaylist(pl)) => {
                    let mut init_url: Option<Url> = None;
                    let mut segments = Vec::with_capacity(pl.segments.len());
                    let mut segment_start_ms = Vec::with_capacity(pl.segments.len());

                    let mut cursor_ms: u64 = 0;
                    for segment in &pl.segments {
                        if init_url.is_none() {
                            if let Some(map) = &segment.map {
                                init_url = Some(
                                    url.join(&map.uri)
                                        .context("failed to resolve init segment url")?,
                                );
                            }
                        }

                        let seg_url = url.join(&segment.uri).with_context(|| {
                            format!("failed to resolve media segment url {}", segment.uri)
                        })?;

                        let duration_ms = (segment.duration as f64 * 1000.0).round() as u64;
                        segment_start_ms.push(cursor_ms);
                        cursor_ms = cursor_ms.saturating_add(duration_ms.max(1));

                        segments.push(HlsSegment {
                            url: seg_url,
                            duration_ms: duration_ms.max(1),
                        });
                    }

                    if segments.is_empty() {
                        return Err(anyhow::anyhow!("HLS media playlist contained no segments"));
                    }

                    return Ok(HlsManifest {
                        init_url,
                        segments,
                        segment_start_ms,
                        total_duration_ms: cursor_ms.max(1),
                    });
                }
                Ok(Playlist::MasterPlaylist(pl)) => {
                    let best = pl
                        .variants
                        .iter()
                        .max_by_key(|v| v.bandwidth)
                        .ok_or_else(|| anyhow::anyhow!("HLS master playlist contained no variants"))?;
                    url = url.join(&best.uri).with_context(|| {
                        format!("failed to resolve variant playlist url {}", best.uri)
                    })?;
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to parse M3U8 playlist: {}", e)),
            }
        }

        Err(anyhow::anyhow!("Too many playlist indirections (master -> media)"))
    }
}
