use reqwest::blocking::Client;

use crate::auth::try_refresh_token;

use super::super::utils::{format_duration, format_playback_count, parse_next_href, parse_str, parse_u64};
use crate::api::{API, Track};

impl API {
    pub fn get_liked_tracks(&mut self) -> anyhow::Result<Vec<Track>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        if self.liked_tracks_next_href.is_none() && !self.first_liked_tracks_page_fetched {
            self.first_liked_tracks_page_fetched = true;
        } else if self.liked_tracks_next_href.is_none() {
            return Ok(Vec::new());
        }

        let url = self.liked_tracks_next_href.clone().unwrap_or_else(|| {
            "https://api.soundcloud.com/me/likes/tracks?limit=40&access=playable,preview,blocked&linked_partitioning=true"
                .to_string()
        });

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        drop(token_guard);

        self.liked_tracks_next_href = parse_next_href(&resp);

        let mut tracks = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for track in collection {
                let title = parse_str(track, "title");

                let artists = parse_str(track, "metadata_artist");
                let artists = if !artists.is_empty() {
                    artists
                } else {
                    parse_str(
                        track.get("user").unwrap_or(&serde_json::Value::Null),
                        "username",
                    )
                };
                let duration = format_duration(parse_u64(track, "duration"));
                let duration_ms = parse_u64(track, "duration");

                let playback_count = parse_u64(track, "playback_count");
                let playback_count = format_playback_count(playback_count);

                let artwork_url = parse_str(track, "artwork_url");

                let stream_url = parse_str(track, "stream_url");
                let access = parse_str(track, "access");
                let track_urn = parse_str(track, "urn");

                tracks.push(Track {
                    title,
                    artists,
                    duration,
                    duration_ms,
                    playback_count,
                    artwork_url,
                    stream_url,
                    access,
                    track_urn,
                });
            }
        }

        Ok(tracks)
    }
}
