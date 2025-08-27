use crate::auth::Token;
use reqwest::blocking::Client;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Track {
    pub title: String,
    pub artists: String,
    pub duration: String,
    pub playback_count: String,
    pub stream_url: String,
}

pub struct API {
    token: Arc<Mutex<Token>>,
    liked_tracks_next_href: Option<String>,
}

fn format_playback_count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.2}K", n as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.2}M", n as f64 / 1_000_000.0),
        1_000_000_000..=999_999_999_999 => format!("{:.2}B", n as f64 / 1_000_000_000.0),
        _ => format!("{:.2}T", n as f64 / 1_000_000_000_000.0),
    }
}

fn format_duration(duration_ms: u64) -> String {
    let duration_sec = duration_ms / 1000;
    let hours = duration_sec / 3600;
    let minutes = (duration_sec % 3600) / 60;
    let seconds = duration_sec % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

impl API {
    pub fn init(token: Arc<Mutex<Token>>) -> Self {
        Self {
            token,
            liked_tracks_next_href: None,
        }
    }

    pub fn get_liked_tracks(&mut self) -> anyhow::Result<Vec<Track>> {
        let token_guard = self.token.lock().unwrap();

        let url = self.liked_tracks_next_href.clone().unwrap_or_else(|| {
        "https://api.soundcloud.com/me/likes/tracks?limit=40&access=playable&linked_partitioning=true".to_string()
    });

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        self.liked_tracks_next_href = resp
            .get("next_href")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut tracks = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for track in collection {
                let title = track
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let artists = track
                    .get("metadata_artist")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        track
                            .get("user")
                            .and_then(|u| u.get("username"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "Unknown Artist".to_string());

                let duration_str =
                    format_duration(track.get("duration").and_then(|v| v.as_u64()).unwrap_or(0));

                let playback_count = track
                    .get("playback_count")
                    .and_then(|v| v.as_u64())
                    .map(format_playback_count)
                    .unwrap_or_else(|| "0".to_string());

                let stream_url = track
                    .get("stream_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let t = Track {
                    title,
                    artists,
                    duration: duration_str,
                    playback_count,
                    stream_url,
                };

                tracks.push(t);
            }
        }

        Ok(tracks)
    }
}
